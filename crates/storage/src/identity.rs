//! S06 persistent instance identity and lifecycle engine.
//!
//! The R03 contract decides *what* an instance is — normalisation, matching precedence, the four
//! outcomes, and the lifecycle rules — as pure functions over a lookup. This module is the other
//! half: the lookup reads the stored instances and source bindings, and the decision is then
//! written back, all inside the caller's transaction.
//!
//! Two rules the code below is arranged around:
//!
//!   * **Nothing is allocated on a guess.** An ambiguous or unresolved candidate writes no instance
//!     row. Choosing one of several candidates would invent a merge that no source stated.
//!   * **A source binding is what keeps a provisional identity from forking.** The second round of
//!     a source that cannot state a complete key must find the instance the first round created,
//!     and it can only do that through the record it was bound to.
use anyhow::Result;
use pcscope_contracts::identity::{
    lifecycle, resolve, ConfirmedIdentity, IdentityRequest, IdentityResolution, InstanceIdentity, InstanceLookup,
    LifecycleHints, LifecycleState, LifecycleTransition, ObservationFacts, ProvisionalIdentity,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;

/// What applying one identity request produced.
#[derive(Debug, Clone)]
pub struct Outcome {
    pub resolution: IdentityResolution,
    /// The stored instance, when one was found or created.
    pub instance_id: Option<u64>,
    pub transition: LifecycleTransition,
    /// True only when this call created the row, so a caller can tell a new instance from a
    /// re-observation without inferring it from timing.
    pub created: bool,
}

fn state_name(state: LifecycleState) -> &'static str {
    match state {
        LifecycleState::Active => "active",
        LifecycleState::NotObserved => "not_observed",
        LifecycleState::Ended => "ended",
        LifecycleState::Removed => "removed",
    }
}

fn state_from(name: &str) -> LifecycleState {
    match name {
        "not_observed" => LifecycleState::NotObserved,
        "ended" => LifecycleState::Ended,
        "removed" => LifecycleState::Removed,
        _ => LifecycleState::Active,
    }
}

/// Reads stored instances for the resolver. Scoped by type, scope and native key, so an instance
/// that happens to share a name under a different scope is never merged with this one.
pub struct DbLookup<'a> {
    pub conn: &'a Connection,
    request: &'a IdentityRequest,
    require_native_key: bool,
}

impl InstanceLookup for DbLookup<'_> {
    fn by_key(&self, type_id: &str, machine_id: &str, installation_id: &str, scope: &str, native_key: &str) -> Vec<(u64, u32)> {
        let mut statement = match self.conn.prepare(
            "SELECT instance_id,incarnation FROM object_instances
             WHERE type_id=? AND machine_id=? AND installation_id=? AND scope=? AND native_key=?
             ORDER BY incarnation,instance_id",
        ) {
            Ok(statement) => statement,
            Err(_) => return Vec::new(),
        };
        let rows = statement.query_map(params![type_id, machine_id, installation_id, scope, native_key], |row| Ok((row.get(0)?, row.get(1)?)));
        match rows {
            Ok(rows) => rows.filter_map(std::result::Result::ok).collect(),
            Err(_) => Vec::new(),
        }
    }

    /// A source's record id is only unique inside its business scope. In particular, providers,
    /// users and different installations can emit the same item/native id. Both the binding and
    /// its object must belong to this request; an old cross-scope binding is historical evidence,
    /// not authority to join the scopes again. Ordinary records can still refine or rename a key.
    fn by_source_binding(&self, source: &str, item_id: &str, native_record_id: &str) -> Option<(u64, u32)> {
        let native_key = if self.require_native_key {
            pcscope_contracts::identity::normalise(&self.request.strategy, &self.request.candidate).0
        } else { String::new() };
        self.conn
            .query_row(
                "SELECT b.instance_id,i.incarnation FROM source_bindings b
                 JOIN object_instances i ON i.instance_id=b.instance_id
                 WHERE b.source=? AND b.item_id=? AND b.native_record_id=? AND b.valid_to IS NULL
                   AND b.scope=? AND i.type_id=? AND i.machine_id=? AND i.installation_id=? AND i.scope=?
                   AND (?=0 OR i.native_key=?)
                 ORDER BY b.binding_id DESC LIMIT 1",
                params![source, item_id, native_record_id, self.request.candidate.scope,
                    self.request.candidate.type_id, self.request.machine_id, self.request.installation_id,
                    self.request.candidate.scope, self.require_native_key, native_key],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .ok()
            .flatten()
    }
}

/// Applies one identity request: resolves it against the store, writes the instance, the binding
/// and the lifecycle transition, and reports what it decided.
///
/// Re-applying the same request changes nothing and returns the same instance, which is what makes
/// a replayed batch harmless.
pub fn apply(tx: &Connection, request: &IdentityRequest) -> Result<Outcome> {
    // These two collectors identify each record by the full task path. Renaming a task gives it
    // a different record id. Older adapters mistakenly stored the folder alone as its key; that
    // historical binding must not override the corrected full path (or combine sibling tasks).
    // Process item ids also contain the creation time. The old reference adapter omitted that
    // time from the native key, merging reused PIDs. Its bindings must not confirm that mixed
    // historical object when the corrected adapter supplies one particular process generation.
    let require_native_key = (request.candidate.type_id == "startup.scheduled_task"
        && matches!(request.binding.source.as_str(), "pcscope.startup.tasks" | "pcscope.startup.task-details"))
        || (request.candidate.type_id == "runtime.process" && request.binding.source == "pcscope.execution.processes");
    let lookup = DbLookup { conn: tx, request, require_native_key };
    apply_using(tx, request, &lookup, false)
}

/// A list item can state several independently identified objects. Its parent record identifies
/// the list, not any particular child, so a binding may only corroborate the child's own key.
/// Ordinary one-object records keep the source-binding precedence used by `apply`.
pub(crate) fn apply_expanded(tx: &Connection, request: &IdentityRequest) -> Result<Outcome> {
    let lookup = DbLookup { conn: tx, request, require_native_key: true };
    apply_using(tx,request,&lookup,true)
}

/// Attributes an event without treating a historical mention as a new observation of a live
/// object. The caller supplies a record reference qualified by the full event evidence address
/// (including its log epoch), and still arbitrates each event-derived field by effective time.
/// Existing identities are never reincarnated here. Deletion can end an existing active object
/// only strictly after its latest known presence/absence; it cannot create a live object.
pub(crate) fn apply_event(tx: &Connection, request: &IdentityRequest) -> Result<Outcome> {
    let lookup = DbLookup { conn: tx, request, require_native_key: true };
    let resolution = resolve(request, &lookup);
    let (identity, status, candidate_instance) = match &resolution {
        IdentityResolution::Confirmed(value) => (value.identity.clone(), "confirmed", value.instance_id),
        IdentityResolution::Provisional(value) => (value.identity.clone(), "provisional", value.instance_id),
        _ => return Ok(Outcome {
            resolution, instance_id: None, created: false,
            transition: LifecycleTransition::Unchanged { reason: "event target could not be uniquely attributed".into() },
        }),
    };
    let event_time = request.evidence_time.source_time.as_deref()
        .and_then(|raw|pcscope_contracts::semantics::canonical_time(raw).ok());
    let mut transition = LifecycleTransition::Unchanged {
        reason: "an attributed event does not assert a new live generation".into(),
    };
    let (instance_id, created) = if let Some(id) = candidate_instance {
        let (state, last_seen, last_absent): (String, String, Option<String>) = tx.query_row(
            "SELECT lifecycle,last_observed_at,last_absent_at FROM object_instances WHERE instance_id=?",
            [id], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)),
        )?;
        // Compare canonical values on BOTH sides, including historical rows without milliseconds.
        // An invalid stored watermark cannot safely be treated as an earlier time.
        let after_known = event_time.as_deref().is_some_and(|at| {
            pcscope_contracts::semantics::canonical_time(&last_seen).ok().is_some_and(|last| at > last.as_str())
                && last_absent.as_deref().is_none_or(|last| {
                    pcscope_contracts::semantics::canonical_time(last).ok().is_some_and(|last| at > last.as_str())
                })
        });
        if after_known && matches!(state.as_str(), "active" | "not_observed") {
            let at = event_time.as_deref().expect("after_known requires a valid time");
            if request.hints.states_object_ended {
                tx.execute("UPDATE object_instances SET lifecycle='ended',last_observed_at=? WHERE instance_id=?", params![at,id])?;
                transition = LifecycleTransition::EndedStated { at: at.into(), basis: "the source stated an end after the latest known presence or absence".into() };
            } else {
                // Update recency, not lifecycle: a log mentioning a previously absent object does
                // not undo an explicit absence, and a log about an ended object never reaches here.
                tx.execute("UPDATE object_instances SET last_observed_at=? WHERE instance_id=?", params![at,id])?;
            }
        }
        (id, false)
    } else {
        if request.hints.states_object_ended || event_time.is_none() {
            return Ok(Outcome {
                resolution, instance_id: None, created: false,
                transition: LifecycleTransition::Unchanged { reason: "a deletion or undated event cannot create a current object".into() },
            });
        }
        let at = event_time.as_deref().expect("a first event requires a valid time");
        transition = LifecycleTransition::FirstObserved { at: at.into(), basis: "first recorded by this event; not an inventory observation".into() };
        (allocate(tx, &identity, status, at, request)?, true)
    };
    bind(tx, instance_id, request, event_time.as_deref().unwrap_or(&request.evidence_time.observed_at))?;
    Ok(Outcome { resolution, instance_id: Some(instance_id), transition, created })
}

fn apply_using(tx: &Connection, request: &IdentityRequest, lookup: &dyn InstanceLookup, expanded: bool) -> Result<Outcome> {
    let resolution = resolve(request, lookup);
    let observed_at = request.evidence_time.observed_at.clone();

    let (identity, status, candidate_instance) = match &resolution {
        IdentityResolution::Confirmed(ConfirmedIdentity { identity, instance_id, .. }) => (identity.clone(), "confirmed", *instance_id),
        IdentityResolution::Provisional(ProvisionalIdentity { identity, instance_id, .. }) => (identity.clone(), "provisional", *instance_id),
        // Several stored instances match and nothing tells them apart. Writing anything here would
        // pick one arbitrarily and merge by accident.
        IdentityResolution::Ambiguous(ambiguity) => {
            let reason = format!("{} existing instances match and nothing distinguishes them; none was chosen", ambiguity.candidates.len());
            return Ok(Outcome {
                resolution,
                instance_id: None,
                transition: LifecycleTransition::Unchanged { reason },
                created: false,
            });
        }
        IdentityResolution::Unresolved { reason } => {
            let reason = reason.clone();
            return Ok(Outcome {
                resolution,
                instance_id: None,
                transition: LifecycleTransition::Unchanged { reason },
                created: false,
            });
        }
    };

    let previous = candidate_instance.and_then(|id| previous_state(tx, id).transpose()).transpose()?;
    let stale = if expanded {
        candidate_instance.map(|id|tx.query_row(
            "SELECT last_observed_at,last_absent_at FROM object_instances WHERE instance_id=?",
            [id], |row|Ok((row.get::<_,String>(0)?,row.get::<_,Option<String>>(1)?)),
        )).transpose()?.is_some_and(|(last_seen,last_absent)|last_seen>observed_at || last_absent.is_some_and(|at|at>=observed_at))
    } else { false };
    let transition = if stale {
        LifecycleTransition::Unchanged { reason: "list evidence does not supersede a later presence or an equally timed absence".into() }
    } else {
        lifecycle(previous, ObservationFacts::Present, &request.hints, &observed_at)
    };

    // A key that comes back after being ended or removed is a new generation, so it gets a new row
    // rather than reviving the old one: the old instance's history belongs to the old object.
    let (instance_id, created) = match (&transition, candidate_instance) {
        (LifecycleTransition::Reincarnated { incarnation, .. }, _) => {
            let mut fresh = identity.clone();
            fresh.incarnation = *incarnation;
            (allocate(tx, &fresh, status, &observed_at, request)?, true)
        }
        (_, Some(id)) => {
            if !stale { touch(tx, id, status, &transition, &observed_at)?; }
            (id, false)
        }
        (_, None) => (allocate(tx, &identity, status, &observed_at, request)?, true),
    };

    bind(tx, instance_id, request, &observed_at)?;
    Ok(Outcome { resolution, instance_id: Some(instance_id), transition, created })
}

fn previous_state(tx: &Connection, instance_id: u64) -> Result<Option<(LifecycleState, u32)>> {
    let row: Option<(String, u32)> = tx
        .query_row("SELECT lifecycle,incarnation FROM object_instances WHERE instance_id=?", params![instance_id], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?;
    Ok(row.map(|(state, incarnation)| (state_from(&state), incarnation)))
}

fn display_name(identity: &InstanceIdentity, request: &IdentityRequest) -> Option<String> {
    let _ = identity;
    request
        .candidate
        .fields
        .iter()
        .find(|field| field.key == "name" || field.key == "display_name" || field.key == "path")
        .map(|field| field.value.clone())
}

fn category_of(type_id: &str) -> String {
    pcscope_contracts::model::type_definition(type_id)
        .and_then(|definition| definition["category"].as_str().map(str::to_owned))
        .unwrap_or_else(|| "other".into())
}

fn allocate(tx: &Connection, identity: &InstanceIdentity, status: &str, observed_at: &str, request: &IdentityRequest) -> Result<u64> {
    tx.execute(
        "INSERT INTO object_instances(type_id,category,machine_id,installation_id,scope,native_key,incarnation,display_name,identity_status,lifecycle,created_at,first_observed_at,last_observed_at)
         VALUES(?,?,?,?,?,?,?,?,?,'active',NULL,?,?)
         ON CONFLICT(type_id,machine_id,installation_id,scope,native_key,incarnation) DO NOTHING",
        params![
            identity.type_id,
            category_of(&identity.type_id),
            identity.machine_id,
            identity.installation_id,
            identity.scope,
            identity.native_key,
            identity.incarnation,
            display_name(identity, request),
            status,
            observed_at,
            observed_at
        ],
    )?;
    let id: u64 = tx.query_row(
        "SELECT instance_id FROM object_instances WHERE type_id=? AND machine_id=? AND installation_id=? AND scope=? AND native_key=? AND incarnation=?",
        params![identity.type_id, identity.machine_id, identity.installation_id, identity.scope, identity.native_key, identity.incarnation],
        |r| r.get(0),
    )?;
    Ok(id)
}

fn touch(tx: &Connection, instance_id: u64, status: &str, transition: &LifecycleTransition, observed_at: &str) -> Result<()> {
    let (lifecycle, absent): (LifecycleState, Option<&str>) = match transition {
        LifecycleTransition::EndedStated { .. } => (LifecycleState::Ended, None),
        LifecycleTransition::RemovedStated { .. } => (LifecycleState::Removed, None),
        LifecycleTransition::NotObserved { .. } => (LifecycleState::NotObserved, Some(observed_at)),
        _ => (LifecycleState::Active, None),
    };
    // `first_observed_at` is deliberately not touched: it is PCScope's first sighting, which is not
    // a creation time and must not drift forward as observations continue. `created_at` stays NULL
    // unless a source actually stated one.
    tx.execute(
        "UPDATE object_instances SET identity_status=?,lifecycle=?,last_observed_at=MAX(last_observed_at,?),last_absent_at=coalesce(?,last_absent_at) WHERE instance_id=?",
        params![status, state_name(lifecycle), observed_at, absent, instance_id],
    )?;
    Ok(())
}

/// Records that this source record belongs to this instance, with a validity range.
///
/// A source record can have bindings in several independent scopes, or to several expanded
/// children. Only an open binding to this exact resolved object and scope makes this a replay.
/// Historic cross-scope and task-folder bindings remain untouched: corrected observations create
/// their own links, without rewriting old facts, evidence or instance aliases.
fn bind(tx: &Connection, instance_id: u64, request: &IdentityRequest, observed_at: &str) -> Result<()> {
    let binding = &request.binding;
    let open: Option<u64> = tx
        .query_row(
            "SELECT instance_id FROM source_bindings WHERE source=? AND item_id=? AND native_record_id=? AND valid_to IS NULL
               AND scope=? AND instance_id=? ORDER BY binding_id DESC LIMIT 1",
            params![binding.source, binding.item_id, binding.native_record_id, request.candidate.scope, instance_id],
            |r| r.get(0),
        )
        .optional()?;
    if open.is_some() {
        return Ok(());
    }
    tx.execute(
        "INSERT INTO source_bindings(instance_id,source,item_id,scope,native_record_id,valid_from) VALUES(?,?,?,?,?,?)
         ON CONFLICT(instance_id,source,item_id,scope,native_record_id,valid_from) DO NOTHING",
        params![instance_id, binding.source, binding.item_id, request.candidate.scope, binding.native_record_id, observed_at],
    )?;
    Ok(())
}

/// Concludes absence only inside one completely observed parent list. Other records belonging to
/// the same source are outside this observation, even when projection drains them in another batch.
#[allow(clippy::too_many_arguments)]
pub(crate) fn conclude_record_absence(
    tx: &Connection, binding: &pcscope_contracts::identity::SourceRecordRef,
    machine_id: &str, installation_id: &str, scope: &str, child_type: &str,
    hints: &LifecycleHints, observed_at: &str,
) -> Result<Vec<u64>> {
    let candidates: Vec<(u64,String,u32)> = {
        let mut statement=tx.prepare(
            "SELECT DISTINCT i.instance_id,i.lifecycle,i.incarnation FROM object_instances i
             JOIN source_bindings b ON b.instance_id=i.instance_id
             WHERE b.source=? AND b.item_id=? AND b.native_record_id=? AND b.valid_to IS NULL
               AND i.machine_id=? AND i.installation_id=? AND i.scope=? AND i.type_id=?
               AND i.last_observed_at<? AND coalesce(i.last_absent_at,'')<?"
        )?;
        let rows=statement.query_map(params![binding.source,binding.item_id,binding.native_record_id,
            machine_id,installation_id,scope,child_type,observed_at,observed_at],
            |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    let mut absent=Vec::new();
    for (id,state,incarnation) in candidates {
        if let LifecycleTransition::NotObserved {..}=lifecycle(Some((state_from(&state),incarnation)),ObservationFacts::Absent,hints,observed_at) {
            // The complete list proves absence, not a new sighting and not deletion.
            tx.execute("UPDATE object_instances SET lifecycle='not_observed',last_absent_at=? WHERE instance_id=?",params![observed_at,id])?;
            absent.push(id);
        }
    }
    Ok(absent)
}

/// Concludes absence for the instances a complete observation of `scope` did not cover.
///
/// Only instances this source and scope were observing, and only those not seen in this round. The
/// R03 lifecycle rules decide the rest — an incomplete observation, an unknown collected scope or a
/// disabled item all come back as `Unchanged`, because absence is not proof of removal.
pub fn conclude_absence(tx: &Connection, source: &str, scope: &str, hints: &LifecycleHints, observed_at: &str) -> Result<Vec<(u64, LifecycleTransition)>> {
    let candidates: Vec<(u64, String, u32)> = {
        let mut statement = tx.prepare(
            "SELECT DISTINCT i.instance_id,i.lifecycle,i.incarnation FROM object_instances i
             JOIN source_bindings b ON b.instance_id=i.instance_id
             WHERE b.source=? AND b.scope=? AND b.valid_to IS NULL AND i.last_observed_at<?",
        )?;
        let rows = statement.query_map(params![source, scope, observed_at], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    let mut decided = Vec::new();
    for (instance_id, state, incarnation) in candidates {
        let transition = lifecycle(Some((state_from(&state), incarnation)), ObservationFacts::Absent, hints, observed_at);
        if let LifecycleTransition::Unchanged { .. } = transition {
            continue;
        }
        touch(tx, instance_id, "confirmed", &transition, observed_at)?;
        decided.push((instance_id, transition));
    }
    Ok(decided)
}

/// The instance summary a query layer shows, read straight from the stored row.
pub fn summary(tx: &Connection, instance_id: u64) -> Result<Option<Value>> {
    let row = tx
        .query_row(
            "SELECT type_id,category,scope,native_key,incarnation,display_name,identity_status,lifecycle,created_at,first_observed_at,last_observed_at,last_absent_at
             FROM object_instances WHERE instance_id=?",
            params![instance_id],
            |r| {
                Ok(serde_json::json!({
                    "instance_id": instance_id,
                    "type_id": r.get::<_, String>(0)?,
                    "category": r.get::<_, String>(1)?,
                    "scope": r.get::<_, String>(2)?,
                    "native_key": r.get::<_, String>(3)?,
                    "incarnation": r.get::<_, u32>(4)?,
                    "display_name": r.get::<_, Option<String>>(5)?,
                    "identity_status": r.get::<_, String>(6)?,
                    "lifecycle": r.get::<_, String>(7)?,
                    "created_at": r.get::<_, Option<String>>(8)?,
                    "first_observed_at": r.get::<_, String>(9)?,
                    "last_observed_at": r.get::<_, String>(10)?,
                    "last_absent_at": r.get::<_, Option<String>>(11)?,
                }))
            },
        )
        .optional()?;
    Ok(row)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Store;
    use pcscope_contracts::adapter::IdentityField;
    use pcscope_contracts::identity::{EvidenceTime, SourceRecordRef, TypeStrategy};

    const TYPE: &str = "startup.scheduled_task";

    fn declared_keys(type_id: &str) -> Vec<String> {
        pcscope_contracts::model::type_definition(type_id)
            .and_then(|d| d["identity"]["keys"].as_array().cloned())
            .map(|keys| keys.iter().filter_map(|k| k.as_str().map(str::to_owned)).collect())
            .unwrap_or_default()
    }

    /// Builds a request the way an adapter would: the `scope` key comes from the collection scope,
    /// the rest from the material. A key with no value is declared missing rather than defaulted.
    fn request(scope: &str, values: &[(&str, &str)], source: &str, record: &str, at: &str) -> IdentityRequest {
        typed_request(TYPE, scope, values, source, record, at)
    }

    fn typed_request(type_id: &str, scope: &str, values: &[(&str, &str)], source: &str, record: &str, at: &str) -> IdentityRequest {
        let declared = declared_keys(type_id);
        let fields: Vec<IdentityField> = declared
            .iter()
            .filter_map(|key| {
                let value = if key == "scope" {
                    Some(scope.to_owned())
                } else {
                    values.iter().find(|(k, _)| k == key).map(|(_, v)| (*v).to_owned())
                };
                value.map(|value| IdentityField { key: key.clone(), value, from: "test".into() })
            })
            .collect();
        let missing: Vec<String> = declared.iter().filter(|key| !fields.iter().any(|f| &f.key == *key)).cloned().collect();
        IdentityRequest {
            candidate: pcscope_contracts::adapter::IdentityCandidate { type_id: type_id.into(), scope: scope.into(), fields, missing, ambiguous: None },
            strategy: TypeStrategy::for_type(type_id).expect("type has a strategy"),
            binding: SourceRecordRef { source: source.into(), item_id: "item".into(), native_record_id: record.into() },
            machine_id: "m".into(),
            installation_id: "i".into(),
            evidence_time: EvidenceTime { source_time: None, observed_at: at.into() },
            hints: LifecycleHints { complete_observation: true, collected_scope: scope.into(), configured: true, history_retained_from: None, states_object_ended: false, boot_id: None },
            rule_version: pcscope_contracts::model::catalog_fingerprint(),
        }
    }

    fn path() -> &'static str {
        r"\例子文件夹\例子任务"
    }

    fn instance_count(store: &Store) -> i64 {
        store.conn.query_row("SELECT count(*) FROM object_instances", [], |r| r.get(0)).unwrap()
    }

    fn open_bindings(store: &Store) -> i64 {
        store.conn.query_row("SELECT count(*) FROM source_bindings WHERE valid_to IS NULL", [], |r| r.get(0)).unwrap()
    }

    fn hints(scope: &str) -> LifecycleHints {
        LifecycleHints { complete_observation: true, collected_scope: scope.into(), configured: true, history_retained_from: None, states_object_ended: false, boot_id: None }
    }

    #[test]
    fn two_sources_converge_on_one_instance() {
        let store = Store::open(":memory:").unwrap();
        let first = store.apply_identity(&request("install:i", &[("path", path())], "pcscope.a", "r1", "2026-09-12T00:00:00Z")).unwrap();
        let second = store.apply_identity(&request("install:i", &[("path", path())], "pcscope.b", "r2", "2026-09-12T00:01:00Z")).unwrap();
        assert_eq!(first.instance_id, second.instance_id);
        assert_eq!(instance_count(&store), 1);
        assert_eq!(open_bindings(&store), 2, "each source keeps its own binding to the one instance");
    }

    #[test]
    fn the_same_name_under_another_scope_is_not_merged() {
        let store = Store::open(":memory:").unwrap();
        let a = store.apply_identity(&request("install:i", &[("path", path())], "pcscope.a", "r1", "2026-09-12T00:00:00Z")).unwrap();
        let b = store.apply_identity(&request("user:s-1-5", &[("path", path())], "pcscope.a", "r2", "2026-09-12T00:00:00Z")).unwrap();
        assert_ne!(a.instance_id, b.instance_id);
        assert_eq!(instance_count(&store), 2);
    }

    #[test]
    fn a_source_binding_is_limited_to_its_type_machine_installation_and_scope() {
        let base = request("provider:A/user:one", &[("path", path())], "pcscope.a", "same-native-id", "2026-09-12T00:00:00Z");
        for boundary in ["scope", "type", "machine", "installation"] {
            let store = Store::open(":memory:").unwrap();
            let original = store.apply_identity(&base).unwrap();
            let before = store.instance_summary(original.instance_id.unwrap()).unwrap();
            let mut other = base.clone();
            other.evidence_time.observed_at = "2026-09-12T00:01:00Z".into();
            match boundary {
                "scope" => {
                    other.candidate.scope = "provider:B/user:one".into();
                    other.hints.collected_scope = other.candidate.scope.clone();
                }
                "type" => other = typed_request("startup.service", &base.candidate.scope, &[("name", path())],
                    &base.binding.source, &base.binding.native_record_id, "2026-09-12T00:01:00Z"),
                "machine" => other.machine_id = "another-machine".into(),
                "installation" => other.installation_id = "another-installation".into(),
                _ => unreachable!(),
            }
            let isolated = store.apply_identity(&other).unwrap();
            assert_ne!(original.instance_id, isolated.instance_id, "{boundary}");
            assert_eq!(instance_count(&store), 2, "{boundary}");
            assert_eq!(open_bindings(&store), 2, "both scopes/objects need their own binding: {boundary}");
            assert_eq!(store.instance_summary(original.instance_id.unwrap()).unwrap(), before, "history changed: {boundary}");
            assert_eq!(store.apply_identity(&base).unwrap().instance_id, original.instance_id, "return to original: {boundary}");
            assert_eq!(store.apply_identity(&other).unwrap().instance_id, isolated.instance_id, "replay: {boundary}");
            assert_eq!(open_bindings(&store), 2, "replays must not add bindings: {boundary}");
        }
    }

    #[test]
    fn source_bindings_require_the_exact_native_record_id_and_binding_scope() {
        let store = Store::open(":memory:").unwrap();
        let first = store.apply_identity(&request("scope", &[("path", "\\A")], "pcscope.a", "native-A", "2026-09-12T00:00:00Z")).unwrap();
        let other = request("scope", &[("path", "\\B")], "pcscope.a", "native-B", "2026-09-12T00:01:00Z");
        let second = store.apply_identity(&other).unwrap();
        assert_ne!(first.instance_id, second.instance_id);
        // Simulate a historical binding whose recorded scope disagrees with its stored object.
        // It must not pull a third key into that object simply because the record ids match.
        store.conn.execute("UPDATE source_bindings SET scope='foreign-scope' WHERE instance_id=?", [second.instance_id.unwrap()]).unwrap();
        let mut changed = other.clone();
        changed.candidate.fields.iter_mut().find(|f| f.key == "path").unwrap().value = "\\C".into();
        changed.evidence_time.observed_at = "2026-09-12T00:02:00Z".into();
        let third = store.apply_identity(&changed).unwrap();
        assert_ne!(second.instance_id, third.instance_id);
        assert_eq!(instance_count(&store), 3);
        assert_eq!(open_bindings(&store), 3);
        assert_eq!(store.apply_identity(&changed).unwrap().instance_id, third.instance_id);
    }

    #[test]
    fn scoped_bindings_still_refine_a_provisional_identity_without_forking() {
        let store = Store::open(":memory:").unwrap();
        let pending = request("scope", &[], "pcscope.a", "native-A", "2026-09-12T00:00:00Z");
        let first = store.apply_identity(&pending).unwrap();
        let complete = request("scope", &[("path", "\\A")], "pcscope.a", "native-A", "2026-09-12T00:01:00Z");
        let confirmed = store.apply_identity(&complete).unwrap();
        assert_eq!(confirmed.instance_id, first.instance_id);
        assert!(matches!(confirmed.resolution, IdentityResolution::Confirmed(_)));
        assert_eq!(store.instance_summary(first.instance_id.unwrap()).unwrap().unwrap()["identity_status"], "confirmed");
        assert_eq!(store.apply_identity(&complete).unwrap().instance_id, first.instance_id);
        assert_eq!(instance_count(&store), 1);
        assert_eq!(open_bindings(&store), 1);
    }

    #[test]
    fn corrected_task_paths_do_not_reuse_old_folder_bindings_or_rewrite_their_history() {
        let store = Store::open(":memory:").unwrap();
        let task_request = |name: &str, path: &str, at: &str| {
            let record = format!("\\Folder\\{name}");
            let mut value = request("scope", &[("path", path)], "pcscope.startup.tasks", &record, at);
            value.binding.item_id = record;
            value
        };
        // Reproduce the old adapter: both source records reported the folder as their native key.
        let old_a = task_request("A", "\\Folder\\", "2026-09-12T00:00:00Z");
        let old_b = task_request("B", "\\Folder\\", "2026-09-12T00:01:00Z");
        let folder = store.apply_identity(&old_a).unwrap().instance_id.unwrap();
        assert_eq!(store.apply_identity(&old_b).unwrap().instance_id, Some(folder));
        let old_summary = store.instance_summary(folder).unwrap();
        let old_bindings: Vec<(u64,String,String,String,Option<String>)> = {
            let mut statement = store.conn.prepare("SELECT binding_id,item_id,scope,valid_from,valid_to FROM source_bindings WHERE instance_id=? ORDER BY binding_id").unwrap();
            let rows = statement.query_map([folder], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?))).unwrap();
            rows.collect::<rusqlite::Result<_>>().unwrap()
        };
        let a = task_request("A", "\\Folder\\A", "2026-09-12T00:02:00Z");
        let b = task_request("B", "\\Folder\\B", "2026-09-12T00:02:00Z");
        let fixed_a = store.apply_identity(&a).unwrap().instance_id.unwrap();
        let fixed_b = store.apply_identity(&b).unwrap().instance_id.unwrap();
        assert_ne!(fixed_a, folder);
        assert_ne!(fixed_b, folder);
        assert_ne!(fixed_a, fixed_b);
        assert_eq!(store.apply_identity(&a).unwrap().instance_id, Some(fixed_a));
        assert_eq!(store.apply_identity(&b).unwrap().instance_id, Some(fixed_b));
        let mut details = a.clone();
        details.binding.source = "pcscope.startup.task-details".into();
        assert_eq!(store.apply_identity(&details).unwrap().instance_id, Some(fixed_a));
        assert_eq!(instance_count(&store), 3);
        assert_eq!(store.instance_summary(folder).unwrap(), old_summary);
        let remaining: Vec<(u64,String,String,String,Option<String>)> = {
            let mut statement = store.conn.prepare("SELECT binding_id,item_id,scope,valid_from,valid_to FROM source_bindings WHERE instance_id=? ORDER BY binding_id").unwrap();
            let rows = statement.query_map([folder], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?))).unwrap();
            rows.collect::<rusqlite::Result<_>>().unwrap()
        };
        assert_eq!(remaining, old_bindings, "legacy links remain auditable and are not relabelled");
        assert_eq!(open_bindings(&store), 5, "two historic links, two corrected records and one detail link");
    }

    #[test]
    fn corrected_process_times_do_not_confirm_old_reused_pid_bindings() {
        let store=Store::open(":memory:").unwrap();
        let process=|started:&str,complete:bool| {
            let record=format!("100/{started}");
            let keys=if complete {vec![("pid","100"),("created_at",started)]}else{vec![("pid","100")]};
            let mut value=typed_request("runtime.process","scope",&keys,"pcscope.execution.processes",&record,"2026-09-12T02:00:00Z");
            value.binding.item_id=record;
            value
        };
        let old_first=process("2026-09-12T00:00:00Z",false);
        let old_second=process("2026-09-12T01:00:00Z",false);
        let old=store.apply_identity(&old_first).unwrap().instance_id.unwrap();
        assert_eq!(store.apply_identity(&old_second).unwrap().instance_id,Some(old));
        let old_summary=store.instance_summary(old).unwrap();
        let old_binding_count:i64=store.conn.query_row("SELECT count(*) FROM source_bindings WHERE instance_id=?",[old],|row|row.get(0)).unwrap();
        let first=process("2026-09-12T00:00:00Z",true);
        let second=process("2026-09-12T01:00:00Z",true);
        let fixed_first=store.apply_identity(&first).unwrap().instance_id.unwrap();
        let fixed_second=store.apply_identity(&second).unwrap().instance_id.unwrap();
        assert_ne!(fixed_first,old);
        assert_ne!(fixed_second,old);
        assert_ne!(fixed_first,fixed_second);
        assert_eq!(store.apply_identity(&first).unwrap().instance_id,Some(fixed_first));
        assert_eq!(store.apply_identity(&second).unwrap().instance_id,Some(fixed_second));
        assert_eq!(store.instance_summary(old).unwrap(),old_summary,"the mixed legacy object remains historical and provisional");
        assert_eq!(store.conn.query_row("SELECT count(*) FROM source_bindings WHERE instance_id=?",[old],|row|row.get::<_,i64>(0)).unwrap(),old_binding_count);
        assert_eq!(instance_count(&store),3);
    }

    fn account_event(sid: &str, epoch: &str, at: &str, ended: bool) -> IdentityRequest {
        let record = format!("event:pcscope.eventlog.security|{epoch}|42");
        let mut value = typed_request("account.user", "machine:m", &[("sid", sid)],
            "pcscope.eventlog.security", &record, "2026-10-13T23:00:00Z");
        value.binding.item_id = if ended { "identity.account_deleted" } else { "identity.account_enabled" }.into();
        value.evidence_time.source_time = Some(at.into());
        value.hints.states_object_ended = ended;
        value
    }

    #[test]
    fn first_deletion_event_does_not_create_a_current_object() {
        let store = Store::open(":memory:").unwrap();
        let event = account_event("S-1-5-21-100", "one", "2026-10-13T01:00:00Z", true);
        let result = apply_event(&store.conn, &event).unwrap();
        assert_eq!(result.instance_id, None);
        assert!(!result.created);
        assert_eq!(instance_count(&store), 0);
        assert_eq!(open_bindings(&store), 0);
    }

    #[test]
    fn an_end_event_must_be_strictly_newer_than_canonical_presence_and_absence() {
        for (at, absent) in [
            ("2026-10-13T01:00:00Z", None),
            ("2026-10-13T02:00:00.000Z", None),
            ("2026-10-13T02:30:00Z", Some("2026-10-13T03:00:00Z")),
        ] {
            let store = Store::open(":memory:").unwrap();
            let snapshot = typed_request("account.user", "machine:m", &[("sid", "S-1-5-21-100")],
                "pcscope.identity.accounts", "S-1-5-21-100", "2026-10-13T02:00:00Z");
            let original = store.apply_identity(&snapshot).unwrap().instance_id.unwrap();
            if let Some(absent) = absent {
                store.conn.execute("UPDATE object_instances SET lifecycle='not_observed',last_absent_at=? WHERE instance_id=?", params![absent,original]).unwrap();
            }
            let before = store.instance_summary(original).unwrap();
            let result = apply_event(&store.conn, &account_event("S-1-5-21-100", "one", at, true)).unwrap();
            assert_eq!(result.instance_id, Some(original));
            assert!(matches!(result.transition, LifecycleTransition::Unchanged { .. }));
            assert_eq!(store.instance_summary(original).unwrap(), before, "{at}");
            assert_eq!(instance_count(&store), 1);
        }
    }

    #[test]
    fn repeated_end_events_never_reincarnate_an_ended_object() {
        let store = Store::open(":memory:").unwrap();
        let first = account_event("S-1-5-21-100", "one", "2026-10-13T01:00:00Z", false);
        let original = apply_event(&store.conn, &first).unwrap().instance_id.unwrap();
        let end = account_event("S-1-5-21-100", "one", "2026-10-13T02:00:00Z", true);
        assert!(matches!(apply_event(&store.conn, &end).unwrap().transition, LifecycleTransition::EndedStated { .. }));
        let ended = store.instance_summary(original).unwrap();
        for epoch in ["one", "two"] {
            let repeat = account_event("S-1-5-21-100", epoch, "2026-10-13T02:00:00.000Z", true);
            let result = apply_event(&store.conn, &repeat).unwrap();
            assert_eq!(result.instance_id, Some(original));
            assert!(!result.created);
            assert!(matches!(result.transition, LifecycleTransition::Unchanged { .. }));
        }
        assert_eq!(store.instance_summary(original).unwrap(), ended);
        assert_eq!(instance_count(&store), 1);
        assert_eq!(ended.unwrap()["incarnation"], 1);
    }

    #[test]
    fn event_mentions_never_reactivate_absent_ended_or_removed_objects() {
        for state in ["not_observed", "ended", "removed"] {
            let store = Store::open(":memory:").unwrap();
            let first = account_event("S-1-5-21-100", "one", "2026-10-13T01:00:00Z", false);
            let original = apply_event(&store.conn, &first).unwrap().instance_id.unwrap();
            store.conn.execute("UPDATE object_instances SET lifecycle=?,last_absent_at='2026-10-13T02:00:00Z' WHERE instance_id=?", params![state,original]).unwrap();
            let later = account_event("S-1-5-21-100", "two", "2026-10-13T03:00:00Z", false);
            assert_eq!(apply_event(&store.conn, &later).unwrap().instance_id, Some(original));
            let row = store.instance_summary(original).unwrap().unwrap();
            assert_eq!(row["lifecycle"], state);
            assert_eq!(row["incarnation"], 1);
            assert_eq!(row["last_observed_at"], if state == "not_observed" { "2026-10-13T03:00:00.000Z" } else { "2026-10-13T01:00:00.000Z" });
            assert_eq!(instance_count(&store), 1);
        }
    }

    #[test]
    fn different_event_epochs_and_target_sids_do_not_share_an_instance() {
        let store = Store::open(":memory:").unwrap();
        let first = account_event("S-1-5-21-100", "one", "2026-10-13T01:00:00Z", false);
        let second = account_event("S-1-5-21-200", "two", "2026-10-13T02:00:00Z", false);
        let a = apply_event(&store.conn, &first).unwrap().instance_id;
        let b = apply_event(&store.conn, &second).unwrap().instance_id;
        assert_ne!(a, b);
        assert_eq!(apply_event(&store.conn, &first).unwrap().instance_id, a);
        assert_eq!(apply_event(&store.conn, &second).unwrap().instance_id, b);
        // Even a legacy unqualified/colliding record reference cannot override a different key.
        let mut legacy = second;
        legacy.binding = first.binding.clone();
        assert_eq!(apply_event(&store.conn, &legacy).unwrap().instance_id, b);
        assert_eq!(instance_count(&store), 2);
    }

    #[test]
    fn re_applying_a_request_changes_nothing() {
        let store = Store::open(":memory:").unwrap();
        let request = request("install:i", &[("path", path())], "pcscope.a", "r1", "2026-09-12T00:00:00Z");
        let first = store.apply_identity(&request).unwrap();
        let again = store.apply_identity(&request).unwrap();
        assert_eq!(first.instance_id, again.instance_id);
        assert!(first.created);
        assert!(!again.created, "a re-observation must not report a new instance");
        assert_eq!(instance_count(&store), 1);
        assert_eq!(open_bindings(&store), 1);
    }

    #[test]
    fn an_incomplete_key_stays_provisional_and_does_not_fork() {
        let store = Store::open(":memory:").unwrap();
        // No path stated: the identity cannot be confirmed, but the source record still binds it.
        let first = store.apply_identity(&request("install:i", &[], "pcscope.a", "r1", "2026-09-12T00:00:00Z")).unwrap();
        assert!(matches!(first.resolution, IdentityResolution::Provisional(_)));
        let second = store.apply_identity(&request("install:i", &[], "pcscope.a", "r1", "2026-09-12T00:01:00Z")).unwrap();
        assert_eq!(first.instance_id, second.instance_id);
        assert_eq!(instance_count(&store), 1);
        let summary = store.instance_summary(first.instance_id.unwrap()).unwrap().unwrap();
        assert_eq!(summary["identity_status"], "provisional");
    }

    #[test]
    fn a_creation_time_is_never_invented() {
        let store = Store::open(":memory:").unwrap();
        let outcome = store.apply_identity(&request("install:i", &[("path", path())], "pcscope.a", "r1", "2026-09-12T00:00:00Z")).unwrap();
        let summary = store.instance_summary(outcome.instance_id.unwrap()).unwrap().unwrap();
        // PCScope's first sighting is recorded as such; the creation time stays unknown because no
        // source stated one, and a later observation must not move it either.
        assert_eq!(summary["created_at"], Value::Null);
        assert_eq!(summary["first_observed_at"], "2026-09-12T00:00:00Z");
        store.apply_identity(&request("install:i", &[("path", path())], "pcscope.a", "r1", "2026-09-12T05:00:00Z")).unwrap();
        let later = store.instance_summary(outcome.instance_id.unwrap()).unwrap().unwrap();
        assert_eq!(later["first_observed_at"], "2026-09-12T00:00:00Z");
        assert_eq!(later["last_observed_at"], "2026-09-12T05:00:00Z");
    }

    #[test]
    fn an_incomplete_observation_cannot_conclude_absence() {
        let store = Store::open(":memory:").unwrap();
        let outcome = store.apply_identity(&request("install:i", &[("path", path())], "pcscope.a", "r1", "2026-09-12T00:00:00Z")).unwrap();
        let id = outcome.instance_id.unwrap();

        let mut incomplete = hints("install:i");
        incomplete.complete_observation = false;
        assert!(store.conclude_absence("pcscope.a", "install:i", &incomplete, "2026-09-12T01:00:00Z").unwrap().is_empty());

        // Nor can a disabled collection item, or a scope the round did not actually cover.
        let mut disabled = hints("install:i");
        disabled.configured = false;
        assert!(store.conclude_absence("pcscope.a", "install:i", &disabled, "2026-09-12T01:00:00Z").unwrap().is_empty());
        let mut unnamed = hints("");
        unnamed.collected_scope = String::new();
        assert!(store.conclude_absence("pcscope.a", "install:i", &unnamed, "2026-09-12T01:00:00Z").unwrap().is_empty());
        assert_eq!(store.instance_summary(id).unwrap().unwrap()["lifecycle"], "active");

        // A complete observation that did cover the scope moves it — and absence is still not
        // deletion, so the instance is not removed.
        let decided = store.conclude_absence("pcscope.a", "install:i", &hints("install:i"), "2026-09-12T01:00:00Z").unwrap();
        assert_eq!(decided.len(), 1);
        let summary = store.instance_summary(id).unwrap().unwrap();
        assert_eq!(summary["lifecycle"], "not_observed");
        assert_eq!(summary["last_absent_at"], "2026-09-12T01:00:00Z");
    }

    #[test]
    fn a_key_that_reappears_after_removal_is_a_new_generation() {
        let store = Store::open(":memory:").unwrap();
        let outcome = store.apply_identity(&request("install:i", &[("path", path())], "pcscope.a", "r1", "2026-09-12T00:00:00Z")).unwrap();
        let original = outcome.instance_id.unwrap();

        // The source states the object ended while still reporting it: R03 calls that `EndedStated`
        // for a present observation, so the instance is ended rather than removed.
        let mut ended = request("install:i", &[("path", path())], "pcscope.a", "r1", "2026-09-12T01:00:00Z");
        ended.hints.states_object_ended = true;
        store.apply_identity(&ended).unwrap();
        assert_eq!(store.instance_summary(original).unwrap().unwrap()["lifecycle"], "ended");

        let reappeared = store.apply_identity(&request("install:i", &[("path", path())], "pcscope.a", "r1", "2026-09-12T02:00:00Z")).unwrap();
        assert!(matches!(reappeared.transition, LifecycleTransition::Reincarnated { .. }));
        assert_ne!(reappeared.instance_id, Some(original));
        assert_eq!(instance_count(&store), 2);
        let summary = store.instance_summary(reappeared.instance_id.unwrap()).unwrap().unwrap();
        assert_eq!(summary["incarnation"], 2);
        assert_eq!(summary["lifecycle"], "active");
    }

    #[test]
    fn an_ambiguous_candidate_writes_nothing() {
        let store = Store::open(":memory:").unwrap();
        let mut ambiguous = request("install:i", &[("path", path())], "pcscope.a", "r1", "2026-09-12T00:00:00Z");
        ambiguous.candidate.ambiguous = Some("two tasks share this name and path".into());
        let outcome = store.apply_identity(&ambiguous).unwrap();
        assert!(outcome.instance_id.is_none());
        assert_eq!(instance_count(&store), 0);
    }

    #[test]
    fn a_source_record_keeps_its_instance_when_the_key_changes() {
        let store = Store::open(":memory:").unwrap();
        let first = store.apply_identity(&request("install:i", &[("path", r"\a")], "pcscope.a", "r1", "2026-09-12T00:00:00Z")).unwrap();
        // The same source record now reports a different path. R03 resolves a source binding before
        // the key, because the source said these records are one record; that statement outlives a
        // key changing, so this is a re-observation of the same instance, not a new one.
        let second = store.apply_identity(&request("install:i", &[("path", r"\b")], "pcscope.a", "r1", "2026-09-12T00:01:00Z")).unwrap();
        assert_eq!(first.instance_id, second.instance_id);
        assert_eq!(instance_count(&store), 1);
        assert_eq!(open_bindings(&store), 1);
        // A second source reporting the new path still finds the instance through the key it
        // matches, so the two views converge rather than forking.
        let third = store.apply_identity(&request("install:i", &[("path", r"\b")], "pcscope.b", "r2", "2026-09-12T00:02:00Z")).unwrap();
        assert_ne!(third.instance_id, first.instance_id, "the changed key is a different instance for a source that never bound the record");
    }
}
