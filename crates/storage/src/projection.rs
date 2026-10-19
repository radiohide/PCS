//! S07 fact, action and current-value projection.
//!
//! This is the real engine behind the stand-in R09 built to prove the chain: the queue S05 fills is
//! drained here, each item goes through an adapter and the identity engine, and what comes out is
//! written as field facts, field history and actions.
//!
//! The rules are not restated here. Which value is current comes from `semantics::select_current`,
//! which action a field change produces from `semantics::change_action`, whether two records are
//! one action from `semantics::merge_action`, and whether an observation states a value at all from
//! `semantics::states_a_value` — the same predicates the drill uses, so the stand-in and the engine
//! cannot drift into two different readings of the same contract.
//!
//! Three properties the code is arranged around:
//!
//!   * **A late, older observation never rolls the current value back.** The stored effective time
//!     decides, not arrival order.
//!   * **An unchanged observation produces no change action.** A field is only a change when both
//!     sides state a value and they take effect at different times.
//!   * **Processing the same evidence twice is the same as processing it once.** Each application
//!     records the evidence it used, and an already-recorded pair is skipped.
use anyhow::Result;
use pcscope_contracts::adapter::{ActionCandidate, ActionOutcome, ActionTarget, AdapterContext, EvidenceInput, FieldCandidate, TimePrecision};
use pcscope_contracts::reference::adapter_for;
use pcscope_contracts::semantics::{business_scope, change_action, effective_time, merge_action, select_current, states_a_value, CurrentValue, FactObservation, MergeDecision};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

use crate::{evidence, identity};

/// What one drain of the queue did.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Report {
    /// Queue entries looked at.
    pub examined: usize,
    /// Entries that produced facts or actions.
    pub projected: usize,
    /// Entries whose source has no adapter yet: recorded as such, not silently dropped.
    pub unadapted: usize,
    /// Entries whose candidates were rejected by the contract validator.
    pub rejected: usize,
    /// Entries that recorded an occurrence the store already held.
    pub duplicate: usize,
    /// Entries whose adapter said the material is not an object at all — the application's own
    /// activity, or a source's material that describes nothing. Not a failure: nothing was wrong
    /// with it, and reporting it as one would fill the health page with the application describing
    /// itself.
    pub skipped: usize,
}

/// Drains up to `limit` pending queue entries.
///
/// Work is taken in queue order and each entry is finished or marked failed before the next is
/// read, so a single unreadable item cannot hold up the ones behind it.
pub fn project_pending(tx: &Connection, machine_id: &str, installation_id: &str, limit: usize) -> Result<Report> {
    let pending: Vec<(i64, String, String)> = {
        let mut statement = tx.prepare(
            "SELECT queue_id,evidence_id,item_id FROM projection_queue
             WHERE projection=? AND state='pending' ORDER BY queue_id LIMIT ?",
        )?;
        let rows = statement.query_map(params![crate::evidence::OBJECT_PROJECTION, limit as i64], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    // Which collection items are switched off right now. A disabled item is not a disappearance:
    // the material it already produced is still projected, and its absence from later rounds must
    // not be read as the object having gone away.
    let disabled = disabled_items(tx);
    let mut report = Report::default();
    let mut snapshot_cache = evidence::SnapshotCache::default();
    let rule_version = pcscope_contracts::model::catalog_fingerprint();
    for (queue_id, evidence_id, item_id) in pending {
        report.examined += 1;
        let Some(input) = evidence::input_at_cached(tx, &evidence_id, &item_id, &mut snapshot_cache)? else {
            finish(tx, queue_id, "failed", Some("the evidence this work item points at is no longer stored"))?;
            report.rejected += 1;
            continue;
        };
        let context = AdapterContext {
            catalog_version: pcscope_contracts::model::catalog_version().into(),
            rule_version: rule_version.clone(),
            machine_id: machine_id.into(),
            installation_id: installation_id.into(),
            boot_id: None,
            // Adapters take the identity scope from here, so it is the object's scope rather than
            // the collection's: two sources that cut the same object up differently must land on
            // one instance, and they only do that if this drops the cutting.
            //
            // Material that states no scope of its own — an event batch — gets the installation the
            // collection ran under. It is the only scope it could have: an event names what
            // happened and when, not which slice of the machine was being walked, and without this
            // the run log describing a task lands in a scope of its own beside the task itself.
            collected_scope: if input.source_scope.trim().is_empty() {
                format!("machine:{machine_id}")
            } else {
                business_scope(&input.source_scope)
            },
            complete: true,
        };
        if input.evidence.kind==pcscope_contracts::adapter::EvidenceKind::Record && input.evidence.id.starts_with("snapshot-absence:") {
            let wrote=apply_snapshot_absence(tx,&input,&context,&disabled)?;
            finish(tx,queue_id,"done",None)?;
            if wrote {report.projected+=1;}else{report.skipped+=1;}
            continue;
        }
        if input.evidence.kind==pcscope_contracts::adapter::EvidenceKind::Record && input.evidence.id.starts_with("snapshot-appearance:") {
            if !valid_snapshot_appearance(&input) {
                finish(tx,queue_id,"failed",Some("snapshot addition material is missing or invalid"))?;
                report.rejected+=1;continue;
            }
            let wrote=apply_snapshot_appearance(tx,&input,&context,&disabled)?;
            finish(tx,queue_id,"done",None)?;
            if wrote {report.projected+=1;}else{report.skipped+=1;}
            continue;
        }
        // A source with no adapter is not an error to hide: it is a known gap, and the queue keeps
        // the reason so the health view can show which sources are still unconverted.
        let Some(adapter) = adapter_for(&input.source) else {
            finish(tx, queue_id, "failed", Some(&format!("no adapter for source {}", input.source)))?;
            report.unadapted += 1;
            continue;
        };
        let result = adapter.adapt(&input, &context);
        let violations = pcscope_contracts::adapter::validate(&result, &context);
        if let Some(first) = violations.first() {
            finish(tx, queue_id, "failed", Some(&format!("{} at {}", first.message, first.at)))?;
            report.rejected += 1;
            continue;
        }
        let not_an_object = matches!(result.outcome, pcscope_contracts::adapter::AdapterOutcome::NotAnObject { .. } | pcscope_contracts::adapter::AdapterOutcome::Skipped { .. });
        let applied = apply(tx, &input, &context, &result, &disabled)?;
        if pcscope_contracts::adapters::source_gaps::reading_destination(&input.source).is_some() {
            let unconfigured=input.source=="pcscope.usn.watch-list" && input.payload["active"].as_bool()==Some(false);
            // A configuration read or one resource sample establishes no complete log history.
            tx.execute("UPDATE coverage_scopes SET configured=CASE WHEN ? THEN 0 ELSE configured END,history_state=CASE WHEN history_state IN ('gap','history_pruned') THEN history_state ELSE 'unknown' END,detail=coalesce(?,detail) WHERE source=? AND scope=?",params![unconfigured,input.payload["reason"].as_str(),input.source,input.source_scope])?;
        }
        finish(tx, queue_id, "done", None)?;
        if applied {
            report.projected += 1;
        } else if not_an_object {
            report.skipped += 1;
        } else {
            report.duplicate += 1;
        }
    }
    Ok(report)
}

/// The collection items currently switched off, read from settings.
fn disabled_items(conn: &Connection) -> std::collections::BTreeSet<String> {
    conn.query_row("SELECT value FROM settings WHERE key='item_settings'", [], |r| r.get::<_, String>(0))
        .optional()
        .ok()
        .flatten()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|value| value["disabled"].as_array().map(|items| items.iter().filter_map(|item| item.as_str().map(str::to_owned)).collect()))
        .unwrap_or_default()
}

fn finish(tx: &Connection, queue_id: i64, state: &str, error: Option<&str>) -> Result<()> {
    tx.execute(
        "UPDATE projection_queue SET state=?,attempts=attempts+1,last_error=?,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE queue_id=?",
        params![state, error, queue_id],
    )?;
    Ok(())
}
fn valid_snapshot_appearance(input:&EvidenceInput)->bool {
    input.payload["basis"]=="snapshot"&&input.payload.get("before")==Some(&Value::Null)&&input.payload["after"].is_object()&&input.payload["scope"].as_str()==Some(input.source_scope.as_str())
}
fn record_presence_action(tx:&Connection,instance:u64,input:&EvidenceInput,context:&AdapterContext,action:&str,from:Option<&str>)->Result<bool> {
    let type_id:String=tx.query_row("SELECT type_id FROM object_instances WHERE instance_id=?",[instance],|row|row.get(0))?;
    let until=pcscope_contracts::semantics::canonical_time(&input.observed_at).map_err(anyhow::Error::msg)?;
    let from=from.and_then(|at|pcscope_contracts::semantics::canonical_time(at).ok()).filter(|at|at<&until);
    let existing:Option<i64>=tx.query_row("SELECT action_row FROM object_actions WHERE instance_id=? AND action_id=? AND observed_at=? LIMIT 1",params![instance,action,input.observed_at],|row|row.get(0)).optional()?;
    let row=if let Some(row)=existing{row}else{
        let summary=json!({"snapshot_presence":{"source":input.source,"protocol":input.payload.get("protocol").or_else(||input.payload.get("Protocol")),"state":input.payload.get("State").or_else(||input.payload.get("state"))}});
        tx.execute("INSERT INTO object_actions(instance_id,action_id,target_type,title,result,precision,interval_from,interval_until,observed_at,rule_version,source_record) VALUES(?,?,?,?,?,?,?,?,?,?,?)",params![instance,action,type_id,title_of(action),"observed",if from.is_some(){"interval"}else{"observation_only"},from,if from.is_some(){Some(input.observed_at.as_str())}else{None},input.observed_at,context.rule_version,summary.to_string()])?;
        tx.last_insert_rowid()
    };
    tx.execute("INSERT OR IGNORE INTO evidence_refs(subject_kind,subject_id,evidence_kind,evidence_id) VALUES('action',?,?,?)",params![row.to_string(),kind_name(&input.evidence),input.evidence.id])?;
    Ok(existing.is_none())
}
/// Explicitly complete child collections. Missing/malformed collections are never an empty set.
fn complete_child_types(input:&EvidenceInput)->Vec<&'static str>{
    if !input.decoding.available||!input.decoding.problems.is_empty(){return vec![]}
    let readable=|key:&str|!input.decoding.unreadable_fields.iter().chain(&input.decoding.masked_fields).any(|field|field==key);
    let strings=|key:&str|readable(key)&&input.payload[key].as_array().is_some_and(|values|values.iter().all(|v|v.as_str().is_some_and(|s|!s.trim().is_empty()&&!pcscope_contracts::semantics::is_mask_sentinel(v))));
    let registry=readable("values")&&input.payload["values"].is_object()&&input.payload["path"].is_string()&&input.payload["hive"].is_string()&&input.payload["view"].is_string();
    match input.source.as_str(){
        "pcscope.apps.browser-policies" if registry=>vec!["policy.browser_policy"],
        "pcscope.environment.configuration" if registry&&!input.payload["path"].as_str().unwrap_or_default().split('\\').any(|s|s.eq_ignore_ascii_case("FileExts")||s.eq_ignore_ascii_case("UrlAssociations"))=>vec!["policy.environment_variable"],
        "pcscope.policy.configuration" if registry=>vec!["policy.software_restriction_policy","policy.uac_policy"],
        "pcscope.startup.logon" if readable("values")&&input.payload["values"].is_object()&&input.payload["registry_path"].as_str().is_some_and(|s|!s.trim().is_empty())=>vec!["startup.logon_startup_entry"],
        "pcscope.security.protection-policy" if ["ExclusionPath","ExclusionProcess","ExclusionExtension"].iter().all(|key|strings(key))=>vec!["security.exclusion"],
        "pcscope.files.metadata-security" if readable("streams")&&input.payload["exists"]==true&&input.payload["streams"].as_array().is_some_and(|values|values.iter().all(|v|v["name"].as_str().is_some_and(|s|!s.is_empty())))=>vec!["file.alternate_data_stream"],
        _=>vec![],
    }
}
fn apply_snapshot_appearance(tx:&Connection,input:&EvidenceInput,context:&AdapterContext,disabled:&std::collections::BTreeSet<String>)->Result<bool> {
    if disabled.contains(&crate::item_controls::classify(&input.source,&input.item_id,"",&input.payload["after"])){return Ok(false)}
    let Some(adapter)=adapter_for(&input.source)else{return Ok(false)};
    let mut present=input.clone();present.payload=input.payload["after"].clone();present.evidence.kind=pcscope_contracts::adapter::EvidenceKind::Snapshot;
    let adapted=adapter.adapt(&present,context);
    if !pcscope_contracts::adapter::validate(&adapted,context).is_empty()||!adapted.problems.is_empty(){return Ok(false)}
    let mut wrote=false;let mut seen=std::collections::BTreeSet::new();
    for observation in adapted.observations {
        let candidate=observation.identity;if candidate.ambiguous.is_some()||!observation.complete{continue}
        let Some(strategy)=pcscope_contracts::identity::TypeStrategy::for_type(&candidate.type_id)else{continue};
        let (key,missing)=pcscope_contracts::identity::normalise(&strategy,&candidate);
        let matches:Vec<u64>={
            let mut query=tx.prepare("SELECT DISTINCT i.instance_id FROM source_bindings b JOIN object_instances i ON i.instance_id=b.instance_id WHERE b.source=? AND b.item_id=? AND b.native_record_id=? AND b.scope=? AND i.type_id=? AND i.machine_id=? AND i.installation_id=? AND i.scope=? AND i.first_observed_at=? AND (? OR i.native_key=?) LIMIT 2")?;
            let rows=query.query_map(params![input.source,input.item_id,input.native_record_id,candidate.scope,candidate.type_id,context.machine_id,context.installation_id,candidate.scope,input.observed_at,!missing.is_empty(),key],|row|row.get(0))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        // An extra registry role or another source for an already-known object is not a new object.
        // A provisional endpoint is usable only when its exact retained source binding is unique.
        if matches.len()!=1||!seen.insert(matches[0]){continue}let instance=matches[0];
        let from=input.payload["before_observed_at"].as_str().and_then(|at|pcscope_contracts::semantics::canonical_time(at).ok())
            .filter(|at|pcscope_contracts::semantics::canonical_time(&input.observed_at).ok().is_some_and(|until|at<&until));
        let existing:Option<i64>=tx.query_row("SELECT action_row FROM object_actions WHERE instance_id=? AND action_id='observed.first_seen' AND observed_at=? LIMIT 1",params![instance,input.observed_at],|row|row.get(0)).optional()?;
        let action=if let Some(id)=existing{id}else{
            // Store only the original appearance's small typed summary. Later current state must
            // never relabel an old listening item as a connected socket (or the reverse).
            let field=|id:&str|observation.fields.iter().find(|field|field.field_id==id&&field.status==pcscope_contracts::adapter::ValueStatus::Known).and_then(|field|field.value.clone());
            let summary=json!({"snapshot_presence":{"source":input.source,"protocol":field("protocol"),"state":field("state")}});
            tx.execute("INSERT INTO object_actions(instance_id,unassigned_reason,action_id,target_type,title,result,occurred_at,precision,interval_from,interval_until,observed_at,actor,program,rule_version,source_record) VALUES(?,NULL,'observed.first_seen',?,'新增列表项','observed',NULL,?,?,?,?,NULL,NULL,?,?)",params![instance,candidate.type_id,if from.is_some(){"interval"}else{"observation_only"},from,if from.is_some(){Some(input.observed_at.as_str())}else{None},input.observed_at,context.rule_version,summary.to_string()])?;
            wrote=true;tx.last_insert_rowid()
        };
        tx.execute("INSERT OR IGNORE INTO evidence_refs(subject_kind,subject_id,evidence_kind,evidence_id) VALUES('action',?,'record',?)",params![action.to_string(),input.evidence.id])?;
    }
    Ok(wrote)
}
fn apply_snapshot_absence(tx:&Connection,input:&EvidenceInput,context:&AdapterContext,disabled:&std::collections::BTreeSet<String>)->Result<bool> {
    // Only internal complete-snapshot difference records reach this branch. Never treat a gap,
    // event or arbitrary snapshot item as proof of absence; never turn prior fields into new facts.
    anyhow::ensure!(input.payload["basis"]=="snapshot"&&input.payload["after"].is_null()&&input.payload["scope"].as_str()==Some(input.source_scope.as_str()),"invalid internal snapshot absence material");
    if disabled.contains(&crate::item_controls::classify(&input.source,&input.item_id,"",&input.payload["before"])){return Ok(false)}
    let Some(adapter)=adapter_for(&input.source)else{return Ok(false)};
    let mut prior=input.clone();prior.payload=input.payload["before"].clone();prior.evidence.kind=pcscope_contracts::adapter::EvidenceKind::Snapshot;
    let adapted=adapter.adapt(&prior,context);let mut wrote=false;let mut seen=std::collections::BTreeSet::new();
    for observation in adapted.observations {
        let candidate=observation.identity;
        if candidate.ambiguous.is_some(){continue}
        let Some(strategy)=pcscope_contracts::identity::TypeStrategy::for_type(&candidate.type_id)else{continue};
        let (key,missing)=pcscope_contracts::identity::normalise(&strategy,&candidate);if !missing.is_empty(){continue}
        let matches:Vec<(u64,String)>= {
            let mut query=tx.prepare("SELECT i.instance_id,i.last_observed_at FROM object_instances i WHERE i.type_id=? AND i.machine_id=? AND i.installation_id=? AND i.scope=? AND i.native_key=? AND i.lifecycle='active' AND i.last_observed_at<? AND EXISTS(SELECT 1 FROM source_bindings b WHERE b.instance_id=i.instance_id AND b.source=? AND b.item_id=?)")?;
            let rows=query.query_map(params![candidate.type_id,context.machine_id,context.installation_id,candidate.scope,key,input.observed_at,input.source,input.item_id],|row|Ok((row.get(0)?,row.get(1)?)))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        // The missing record may only update a uniquely identified, already bound object.
        if matches.len()!=1 {continue}let (instance,from)=&matches[0];if !seen.insert(*instance){continue}
        // Another source or registration role can still describe this same object.
        if identity::has_retained_observation(tx,*instance,&input.observed_at,&input.source,&input.item_id)? {continue}
        tx.execute("UPDATE object_instances SET lifecycle='not_observed',last_absent_at=? WHERE instance_id=?",params![input.observed_at,instance])?;
        tx.execute("INSERT INTO object_actions(instance_id,unassigned_reason,action_id,target_type,title,result,occurred_at,precision,interval_from,interval_until,observed_at,actor,program,rule_version) VALUES(?,NULL,'observed.no_longer_observed',?,'本次检查未找到此对象','observed',NULL,'interval',?,?,?,NULL,NULL,?)",params![instance,candidate.type_id,from,input.observed_at,input.observed_at,context.rule_version])?;
        let action=tx.last_insert_rowid();
        for (kind,id) in [("action",action.to_string()),("observation",instance.to_string())] {
            tx.execute("INSERT OR IGNORE INTO evidence_refs(subject_kind,subject_id,evidence_kind,evidence_id) VALUES(?,?,'record',?)",params![kind,id,input.evidence.id])?;
        }
        wrote=true;
    }
    Ok(wrote)
}

/// Applies one adapter result. Returns false when the evidence had already been applied.
fn apply(tx: &Connection, input: &EvidenceInput, context: &AdapterContext, result: &pcscope_contracts::adapter::AdapterResult, disabled: &std::collections::BTreeSet<String>) -> Result<bool> {
    let mut wrote = false;
    // These sources report a whole child set in one parent record. Qualify every candidate,
    // including the group itself, so a newly bound membership cannot take over its parent.
    let children = match input.source.as_str() {
        "pcscope.identity.assigned-rights" => Some(("rights","account.right_assignment")),
        "pcscope.identity.groups" => Some(("members","account.group_membership")),
        _ => None,
    };
    let complete_children=complete_child_types(input);
    // A parent can contain only one child in a later round; never infer this from row count.
    // Absence remains restricted to the independently validated complete-list rules below.
    let expanded = matches!(input.source.as_str(),
        "pcscope.identity.assigned-rights" | "pcscope.identity.groups" |
        "pcscope.files.metadata-security" | "pcscope.network.access" |
        "pcscope.policy.configuration" | "pcscope.apps.browser-policies" |
        "pcscope.environment.configuration" | "pcscope.security.protection-policy" |
        "pcscope.startup.logon" | "pcscope.apps.dotnet-runtimes");
    let scope=pcscope_contracts::adapters::installation_scope(&context.collected_scope);
    // One bounded key lookup per parent, not per candidate and not a scan of evidence history.
    // Keep an explicit complete-list watermark even for an empty list with no child bindings.
    // The internal namespace is not returned by the settings APIs, which read named settings.
    let parent_watermark = if (children.is_some()||!complete_children.is_empty()) && input.evidence.kind == pcscope_contracts::adapter::EvidenceKind::Snapshot {
        Some(format!("projection.expanded_complete_at.v1/{}",crate::digest(&serde_json::to_string(&(
            &input.source,&context.machine_id,&context.installation_id,&scope,&input.item_id,&input.native_record_id,
        ))?)))
    } else {None};
    let previous_parent=parent_watermark.as_ref().map(|key|tx.query_row("SELECT value FROM settings WHERE key=?",[key],|row|row.get::<_,String>(0)).optional()).transpose()?.flatten();
    if previous_parent.as_ref().is_some_and(|at|at>&input.observed_at) {
            // The raw evidence and queue entry remain auditable; an older complete parent list
            // cannot introduce a previously unseen historical child as a current assignment.
            return Ok(false);
    }
    let configured = !disabled.contains(&crate::item_controls::classify(&input.source, &input.item_id, "", &Value::Null));
    for observation in &result.observations {
        let request = pcscope_contracts::identity::IdentityRequest {
            candidate: observation.identity.clone(),
            strategy: match pcscope_contracts::identity::TypeStrategy::for_type(&observation.identity.type_id) {
                Some(strategy) => strategy,
                None => continue,
            },
            binding: pcscope_contracts::identity::SourceRecordRef {
                source: input.source.clone(),
                item_id: input.item_id.clone(),
                native_record_id: input.native_record_id.clone(),
            },
            machine_id: context.machine_id.clone(),
            installation_id: context.installation_id.clone(),
            evidence_time: pcscope_contracts::identity::EvidenceTime { source_time: input.source_time.clone(), observed_at: observation.observed_at.clone() },
            hints: pcscope_contracts::identity::LifecycleHints {
                complete_observation: observation.complete && context.complete,
                collected_scope: context.collected_scope.clone(),
                configured,
                history_retained_from: None,
                states_object_ended: false,
                boot_id: context.boot_id.clone(),
            },
            rule_version: context.rule_version.clone(),
        };
        let outcome = if expanded { identity::apply_expanded(tx, &request)? } else { identity::apply(tx, &request)? };
        let Some(instance_id) = outcome.instance_id else { continue };
        crate::display::bind_scope(tx,instance_id,input)?;
        // A COM snapshot can contain the root, InprocServer32 and LocalServer32 for one
        // instance. Its evidence address names the whole snapshot, so claim each item before
        // applying its distinct fields. Keep the normal instance-level reference for evidence
        // queries/counts; it must not suppress another key role from the same snapshot.
        let first_application = if input.source == "pcscope.shell.registrations"
            && input.evidence.kind == pcscope_contracts::adapter::EvidenceKind::Snapshot
            && observation.identity.type_id == "shell.com_registration" {
            let item_subject = serde_json::to_string(&(instance_id, &input.item_id))?;
            let first = claim(tx, "observation_item", &item_subject, &observation.evidence)?;
            if first { claim(tx, "observation", &instance_id.to_string(), &observation.evidence)?; }
            first
        } else {
            // Other types retain the existing per-instance replay contract.
            claim(tx, "observation", &instance_id.to_string(), &observation.evidence)?
        };
        if !first_application {
            continue;
        }
        if configured && observation.complete && context.complete && input.evidence.kind==pcscope_contracts::adapter::EvidenceKind::Snapshot {
            if observation.identity.type_id=="hardware.driver_binding" {
                let retired=identity::retire_replaced_binding(tx,&request,instance_id)?;
                let mut previous_binding=None;
                for old in &retired {
                    let seen:String=tx.query_row("SELECT last_observed_at FROM object_instances WHERE instance_id=?",[old],|row|row.get(0))?;
                    record_presence_action(tx,*old,input,context,"observed.no_longer_observed",Some(&seen))?;
                    previous_binding=Some(seen);
                }
                if outcome.created && !retired.is_empty() {
                    record_presence_action(tx,instance_id,input,context,"observed.first_seen",previous_binding.as_deref())?;
                }
            }
            if matches!(outcome.transition,pcscope_contracts::identity::LifecycleTransition::ObservedAgain {..}) {
                let absent:Option<String>=tx.query_row("SELECT last_absent_at FROM object_instances WHERE instance_id=?",[instance_id],|row|row.get(0))?;
                record_presence_action(tx,instance_id,input,context,"observed.reappeared",absent.as_deref())?;
            } else if outcome.created && (children.is_some_and(|(_,typ)|typ==observation.identity.type_id)||complete_children.contains(&observation.identity.type_id.as_str())) && previous_parent.is_some() {
                record_presence_action(tx,instance_id,input,context,"observed.first_seen",previous_parent.as_deref())?;
            }
        }
        for field in &observation.fields {
            record_fact(tx, instance_id, input, context, field, &observation.observed_at, &observation.evidence)?;
        }
        wrote = true;
    }
    if let Some((list_key, child_type)) = children {
        // A missing, unreadable or malformed list cannot establish absence. An actual empty
        // list can. Only this record's children are covered, never the whole source's scope.
        let complete_list = input.payload.get(list_key).and_then(Value::as_array)
            .is_some_and(|values|values.iter().all(|value|!pcscope_contracts::semantics::is_mask_sentinel(value)
                && value.as_str().is_some_and(|value|!value.trim().is_empty())));
        let readable = input.decoding.available && input.decoding.problems.is_empty()
            && !input.decoding.unreadable_fields.iter().any(|field|field==list_key)
            && !input.decoding.masked_fields.iter().any(|field|field==list_key);
        if configured && complete_list && readable && context.complete && result.problems.is_empty()
            && result.observations.iter().all(|observation|observation.complete)
            && input.evidence.kind == pcscope_contracts::adapter::EvidenceKind::Snapshot {
            let hints=pcscope_contracts::identity::LifecycleHints {
                complete_observation:true,collected_scope:scope.clone(),configured,
                history_retained_from:None,states_object_ended:false,boot_id:context.boot_id.clone(),
            };
            let binding=pcscope_contracts::identity::SourceRecordRef {
                source:input.source.clone(),item_id:input.item_id.clone(),native_record_id:input.native_record_id.clone(),
            };
            for id in identity::conclude_record_absence(tx,&binding,&context.machine_id,&context.installation_id,&scope,child_type,&hints,&input.observed_at)? {
                wrote |= claim(tx,"observation",&id.to_string(),std::slice::from_ref(&input.evidence))?;
                let seen:String=tx.query_row("SELECT last_observed_at FROM object_instances WHERE instance_id=?",[id],|row|row.get(0))?;
                wrote|=record_presence_action(tx,id,input,context,"observed.no_longer_observed",Some(&seen))?;
            }
            if let Some(key)=&parent_watermark {
                tx.execute("INSERT INTO settings(key,value) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value WHERE settings.value<excluded.value",params![key,input.observed_at])?;
            }
        }
    }
    if configured&&context.complete&&result.problems.is_empty()&&result.observations.iter().all(|o|o.complete)&&input.evidence.kind==pcscope_contracts::adapter::EvidenceKind::Snapshot {
        for typ in &complete_children {
            let scopes:Vec<String>={
                let mut q=tx.prepare("SELECT DISTINCT b.scope FROM source_bindings b JOIN object_instances i ON i.instance_id=b.instance_id JOIN object_collection_scopes c ON c.instance_id=i.instance_id AND c.source=b.source WHERE b.source=? AND b.item_id=? AND b.native_record_id=? AND i.machine_id=? AND i.installation_id=? AND i.type_id=? AND c.scope=? LIMIT 129")?;
                let rows=q.query_map(params![input.source,input.item_id,input.native_record_id,context.machine_id,context.installation_id,typ,input.source_scope],|r|r.get(0))?;rows.collect::<rusqlite::Result<_>>()?
            };
            if scopes.len()>128{continue}
            for child_scope in scopes {
                let hints=pcscope_contracts::identity::LifecycleHints{complete_observation:true,collected_scope:child_scope.clone(),configured,history_retained_from:None,states_object_ended:false,boot_id:context.boot_id.clone()};
                let binding=pcscope_contracts::identity::SourceRecordRef{source:input.source.clone(),item_id:input.item_id.clone(),native_record_id:input.native_record_id.clone()};
                for id in identity::conclude_record_absence(tx,&binding,&context.machine_id,&context.installation_id,&child_scope,typ,&hints,&input.observed_at)? {
                    claim(tx,"observation",&id.to_string(),std::slice::from_ref(&input.evidence))?;
                    let seen:String=tx.query_row("SELECT last_observed_at FROM object_instances WHERE instance_id=?",[id],|r|r.get(0))?;
                    wrote|=record_presence_action(tx,id,input,context,"observed.no_longer_observed",Some(&seen))?;
                }
            }
        }
        if !complete_children.is_empty(){if let Some(key)=&parent_watermark {tx.execute("INSERT INTO settings(key,value) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value WHERE settings.value<excluded.value",params![key,input.observed_at])?;}}
    }
    for action in &result.actions {
        if record_action(tx, input, context, action)? {
            wrote = true;
        }
    }
    Ok(wrote)
}

/// Records that this evidence has been applied to this subject, and reports whether it is new.
fn claim(tx: &Connection, subject_kind: &str, subject_id: &str, evidence: &[pcscope_contracts::adapter::EvidenceRef]) -> Result<bool> {
    let Some(first) = evidence.first() else { return Ok(false) };
    let inserted = tx.execute(
        "INSERT INTO evidence_refs(subject_kind,subject_id,evidence_kind,evidence_id) VALUES(?,?,?,?) ON CONFLICT DO NOTHING",
        params![subject_kind, subject_id, kind_name(first), first.id],
    )?;
    Ok(inserted > 0)
}

fn kind_name(reference: &pcscope_contracts::adapter::EvidenceRef) -> &'static str {
    use pcscope_contracts::adapter::EvidenceKind::*;
    match reference.kind {
        Record => "record",
        Snapshot => "snapshot",
        Event => "event",
    }
}

fn as_fact_observation(input: &EvidenceInput, field: &FieldCandidate, observed_at: &str, evidence: &[pcscope_contracts::adapter::EvidenceRef]) -> FactObservation {
    FactObservation {
        source: input.source.clone(),
        value: field.value.clone(),
        value_type: field.value_type.clone(),
        status: field.status,
        source_time: field.source_time.clone().or_else(|| input.source_time.clone()),
        observed_at: observed_at.to_owned(),
        evidence: evidence.to_vec(),
    }
}
fn normalise_members(input:&EvidenceInput,field:&FieldCandidate,value:&mut Option<Value>){
    if input.source!="pcscope.identity.groups"||field.field_id!="members"{return}
    let Some(values)=value.as_ref().and_then(Value::as_array)else{return};
    let Some(mut members)=values.iter().map(|v|v.as_str().filter(|s|s.to_ascii_lowercase().starts_with("s-")).map(str::to_ascii_lowercase)).collect::<Option<Vec<_>>>()else{return};
    members.sort();members.dedup();*value=Some(json!(members));
}

/// Writes one field's current value, its history and any change action. Returns whether anything
/// moved.
fn record_fact(
    tx: &Connection,
    instance_id: u64,
    input: &EvidenceInput,
    context: &AdapterContext,
    field: &FieldCandidate,
    observed_at: &str,
    evidence: &[pcscope_contracts::adapter::EvidenceRef],
) -> Result<()> {
    let mut now = as_fact_observation(input, field, observed_at, evidence);
    normalise_members(input,field,&mut now.value);
    let stored: Option<(Option<String>, String, Option<String>, String, String)> = tx
        .query_row(
            "SELECT value,value_status,source_time,observed_at,rule_version FROM object_facts WHERE instance_id=? AND field_id=?",
            params![instance_id, field.field_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .optional()?;

    let Some((value, status, source_time, stored_observed, stored_rule)) = stored else {
        upsert_current(tx, instance_id, input, context, field, &now, None)?;
        return Ok(());
    };

    // The stored row is read back as a fact observation and the same selection rule that decides
    // the current value decides what this new observation does to it.
    let mut previous = FactObservation {
        source: String::new(),
        value: value.clone().map(|text| serde_json::from_str(&text).unwrap_or(Value::String(text))),
        value_type: String::new(),
        status: status_from(&status),
        source_time,
        observed_at: stored_observed.clone(),
        evidence: Vec::new(),
    };
    normalise_members(input,field,&mut previous.value);
    // Repeating either side of an unresolved tie is not new evidence that resolves the tie.
    // Only a strictly later, stated value can replace an existing conflict.
    if previous.status == pcscope_contracts::adapter::ValueStatus::Conflicting
        && effective_time(&now) <= effective_time(&previous) {return Ok(());}
    match select_current(&[previous.clone(), now.clone()]) {
        CurrentValue::Known { value: selected, .. } => {
            let before = previous.value.clone().filter(|_| states_a_value(&previous));
            let changed = previous.status == pcscope_contracts::adapter::ValueStatus::Known
                && now.status == pcscope_contracts::adapter::ValueStatus::Known
                && effective_time(&previous) != effective_time(&now)
                && !matches!((&before,&now.value),(Some(before),Some(after)) if pcscope_contracts::sensitive::equivalent(before,after));
            // A late, older observation leaves the current value alone — it is still recorded as
            // history, but it does not become the present.
            let is_current = states_a_value(&now)
                && (!states_a_value(&previous) || effective_time(&now) >= effective_time(&previous));
            if changed && is_current {
                // Old COM rules mixed separate registry-key roles into implementation facts.
                // Their last value is not a trustworthy comparison baseline. Only after normal
                // time/protection selection accepts a newer known value do we establish the new
                // baseline; the observation and its original evidence remain recorded by apply.
                let corrected_com_baseline = input.source == "pcscope.shell.registrations"
                    && input.evidence.kind == pcscope_contracts::adapter::EvidenceKind::Snapshot
                    && matches!(field.field_id.as_str(), "server_path" | "file_version" | "sha256")
                    && crate::query::is_legacy_com_rule(&stored_rule)
                    && !crate::query::is_legacy_com_rule(&context.rule_version)
                    && tx.query_row(
                        "SELECT type_id='shell.com_registration' FROM object_instances WHERE instance_id=?",
                        [instance_id], |row| row.get::<_, bool>(0),
                    ).optional()?.unwrap_or(false);
                if !corrected_com_baseline {
                    let (Some(before), Some(after)) = (before.clone(), now.value.clone()) else { return Ok(()) };
                    write_history(tx, instance_id, input, context, field, &before, &after, &previous, &now)?;
                    // A precise event already has its own timeline entry. Only snapshot comparisons
                    // synthesize a second, interval-based change action.
                    if input.evidence.kind == pcscope_contracts::adapter::EvidenceKind::Snapshot {
                        write_change_action(tx, instance_id, input, context, field, &before, &after, &previous, &now)?;
                    }
                }
            } else if changed {
                // Preserve older material in chronological order, never as a backwards change.
                let (Some(older), Some(newer)) = (now.value.clone(), before.clone()) else { return Ok(()) };
                write_history(tx, instance_id, input, context, field, &older, &newer, &now, &previous)?;
            }
            if is_current {
                upsert_current(tx, instance_id, input, context, field, &now, Some(&selected))?;
            }
            let _ = (changed, is_current);
            Ok(())
        }
        // Sources disagree at one effective time and no rule resolves it: both are kept, and the
        // newer one does not silently win.
        CurrentValue::Conflict { candidates, .. } => {
            let record = json!({
                "candidates": candidates.iter().map(|candidate| json!({"source": candidate.source, "value": candidate.value, "observed_at": candidate.observed_at})).collect::<Vec<_>>(),
            });
            tx.execute(
                "INSERT INTO object_facts(instance_id,field_id,field_kind,value,value_type,value_status,source_time,observed_at,derivation,rule_version,conflict_sources)
                 VALUES(?,?,?,?,'unknown','conflicting',?,?,?,?,?)
                 ON CONFLICT(instance_id,field_id) DO UPDATE SET value_status='conflicting',conflict_sources=excluded.conflict_sources,source_time=excluded.source_time,observed_at=excluded.observed_at",
                params![instance_id, field.field_id, field_kind_name(field), serde_json::to_string(&now.value)?, now.source_time, observed_at, derivation_name(field), context.rule_version, record.to_string()],
            )?;
            Ok(())
        }
        CurrentValue::Absent { .. } | CurrentValue::Refused { .. } => Ok(()),
    }
}

fn status_from(name: &str) -> pcscope_contracts::adapter::ValueStatus {
    use pcscope_contracts::adapter::ValueStatus::*;
    match name {
        "known" => Known,
        "masked" => Masked,
        "conflicting" => Conflicting,
        "unreadable" => Unreadable,
        "evidence_pruned" => EvidencePruned,
        "not_applicable" => NotApplicable,
        _ => NotProvided,
    }
}

fn field_kind_name(field: &FieldCandidate) -> &'static str {
    use pcscope_contracts::adapter::FieldKind::*;
    match field.field_kind {
        Configuration => "configuration",
        State => "state",
        Identity => "identity",
    }
}

fn derivation_name(field: &FieldCandidate) -> &'static str {
    use pcscope_contracts::adapter::Derivation::*;
    match field.derivation {
        SourceProvided => "source_provided",
        Decoded => "decoded",
        SnapshotComparison => "snapshot_comparison",
        Rule => "rule",
    }
}

fn upsert_current(
    tx: &Connection,
    instance_id: u64,
    _input: &EvidenceInput,
    context: &AdapterContext,
    field: &FieldCandidate,
    observation: &FactObservation,
    selected: Option<&Value>,
) -> Result<()> {
    let value = selected.or(observation.value.as_ref()).cloned().unwrap_or(Value::Null);
    // A late historical name must not replace the effective display name.
    if field.field_id == "name" && field.status == pcscope_contracts::adapter::ValueStatus::Known {
        if let Some(name) = value.as_str() {
            tx.execute("UPDATE object_instances SET display_name=? WHERE instance_id=?", params![name, instance_id])?;
        }
    }
    tx.execute(
        "INSERT INTO object_facts(instance_id,field_id,field_kind,value,value_type,value_status,source_time,observed_at,derivation,rule_version,conflict_sources)
         VALUES(?,?,?,?,?,?,?,?,?,?,NULL)
         ON CONFLICT(instance_id,field_id) DO UPDATE SET value=excluded.value,value_type=excluded.value_type,value_status=excluded.value_status,
             source_time=excluded.source_time,observed_at=excluded.observed_at,derivation=excluded.derivation,rule_version=excluded.rule_version,conflict_sources=NULL",
        params![
            instance_id,
            field.field_id,
            field_kind_name(field),
            value.to_string(),
            field.value_type,
            status_name(field.status),
            observation.source_time,
            observation.observed_at,
            derivation_name(field),
            context.rule_version
        ],
    )?;
    Ok(())
}

fn status_name(status: pcscope_contracts::adapter::ValueStatus) -> &'static str {
    use pcscope_contracts::adapter::ValueStatus::*;
    match status {
        Known => "known",
        Masked => "masked",
        Conflicting => "conflicting",
        Unreadable => "unreadable",
        EvidencePruned => "evidence_pruned",
        NotApplicable => "not_applicable",
        NotProvided => "not_provided",
    }
}

#[allow(clippy::too_many_arguments)]
fn write_history(
    tx: &Connection,
    instance_id: u64,
    input: &EvidenceInput,
    context: &AdapterContext,
    field: &FieldCandidate,
    before: &Value,
    after: &Value,
    previous: &FactObservation,
    now: &FactObservation,
) -> Result<()> {
    tx.execute(
        "INSERT INTO fact_history(instance_id,field_id,before_value,before_status,before_source_time,after_value,after_status,after_source_time,observed_from,observed_until,source,rule_version)
         VALUES(?,?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT DO NOTHING",
        params![
            instance_id,
            field.field_id,
            before.to_string(),
            status_name(previous.status),
            previous.source_time,
            after.to_string(),
            status_name(now.status),
            now.source_time,
            effective_time(previous),
            effective_time(now),
            input.source,
            context.rule_version
        ],
    )?;
    Ok(())
}

/// A change of a field observed twice. The interval is the two observation times, never a claimed
/// exact moment: no source recorded the edit, so none is invented.
#[allow(clippy::too_many_arguments)]
fn write_change_action(
    tx: &Connection,
    instance_id: u64,
    input: &EvidenceInput,
    context: &AdapterContext,
    field: &FieldCandidate,
    before: &Value,
    after: &Value,
    previous: &FactObservation,
    now: &FactObservation,
) -> Result<()> {
    let Some(action_id) = change_action(field.field_kind) else { return Ok(()) };
    let from = previous.observed_at.clone();
    let until = now.observed_at.clone();
    if from == until {
        // Two observations that take effect at one instant are an agreement or a conflict, never a
        // span. Writing a zero-length interval would claim a change nobody saw.
        return Ok(());
    }
    let mut evidence: Vec<pcscope_contracts::adapter::EvidenceRef> = previous.evidence.clone();
    evidence.extend(now.evidence.clone());
    evidence.dedup();
    tx.execute(
        "INSERT INTO object_actions(instance_id,unassigned_reason,action_id,target_type,title,result,field_id,before_value,after_value,occurred_at,precision,interval_from,interval_until,observed_at,actor,program,rule_version)
         VALUES(?,NULL,?,?,?,?,?,?,?,NULL,'interval',?,?,?,NULL,NULL,?)",
        params![
            instance_id,
            action_id,
            input.source,
            title_of(action_id),
            "observed",
            field.field_id,
            before.to_string(),
            after.to_string(),
            from,
            until,
            now.observed_at,
            context.rule_version
        ],
    )?;
    let row = tx.last_insert_rowid();
    for reference in evidence {
        tx.execute(
            "INSERT INTO evidence_refs(subject_kind,subject_id,evidence_kind,evidence_id) VALUES('action',?,?,?) ON CONFLICT DO NOTHING",
            params![row.to_string(), kind_name(&reference), reference.id],
        )?;
    }
    Ok(())
}

fn title_of(action_id: &str) -> String {
    pcscope_contracts::model::action_definition(action_id)
        .and_then(|definition| definition["name"].as_str().map(str::to_owned))
        .unwrap_or_else(|| action_id.to_owned())
}

/// Writes an action the adapter reported. Two records of one occurrence are one action with several
/// pieces of evidence, not two rows; the merge rule decides, and time proximity alone never merges.
pub(crate) fn service_state_target(tx: &Connection, input: &EvidenceInput, context: &AdapterContext)
    -> Result<Option<(pcscope_contracts::adapter::IdentityCandidate,pcscope_contracts::event_state::EventStateIntent)>> {
    let service_match = pcscope_contracts::event_state::service_state_match(input, context);
    if let Some(found) = service_match {
        let scope=pcscope_contracts::adapters::installation_scope(&context.collected_scope);
        let mut statement=tx.prepare(
            "SELECT n.value FROM object_instances i
             JOIN object_facts d ON d.instance_id=i.instance_id AND d.field_id='display_name' AND d.value_status='known'
             JOIN object_facts n ON n.instance_id=i.instance_id AND n.field_id='name' AND n.value_status='known'
             WHERE i.type_id='startup.service' AND i.machine_id=? AND i.installation_id=? AND i.scope=?
               AND i.identity_status='confirmed' AND i.lifecycle NOT IN ('removed','ended') AND d.value=? LIMIT 2")?;
        let names=statement.query_map(params![context.machine_id,context.installation_id,scope,json!(found.display_name).to_string()],|row|row.get::<_,String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if names.len()==1 {
            if let Some(name)=serde_json::from_str::<Value>(&names[0])?.as_str().filter(|name|!name.trim().is_empty()) {
                return Ok(Some((pcscope_contracts::adapters::identity("startup.service",&scope,&input.evidence.id,&[("name",Some(name.into()))]),found.intent)));
            }
        }
    }
    Ok(None)
}

fn record_action(tx: &Connection, input: &EvidenceInput, context: &AdapterContext, action: &ActionCandidate) -> Result<bool> {
    let mut qualified = action.clone();
    let service_intent=if let Some((identity,intent))=service_state_target(tx,input,context)? {
        qualified.target=ActionTarget::Assigned {identity};Some(intent)
    } else {None};
    // Imported logs belong to a historical environment. Without an explicit mapping they may
    // not reactivate a local object or create a current instance using this machine's identity.
    if input.source.starts_with("pcscope.eventlog.import.") {
        qualified.target=ActionTarget::Unassigned {reason:"导入的历史日志，未关联到本机当前对象。".into()};
    }
    let action=&qualified;
    if let Some(key) = &action.occurrence_key {
        let existing: Option<i64> = tx.query_row("SELECT action_row FROM object_actions WHERE occurrence_key=? LIMIT 1", [key], |row| row.get(0)).optional()?;
        if let Some(existing) = existing {
            for reference in &action.evidence { tx.execute("INSERT OR IGNORE INTO evidence_refs(subject_kind,subject_id,evidence_kind,evidence_id) VALUES('action',?,?,?)", params![existing.to_string(), kind_name(reference), reference.id])?; }
            return Ok(false);
        }
    }
    let (instance_id, unassigned, target_type) = match &action.target {
        ActionTarget::Assigned { identity } => {
            let request = pcscope_contracts::identity::IdentityRequest {
                candidate: identity.clone(),
                strategy: match pcscope_contracts::identity::TypeStrategy::for_type(&identity.type_id) {
                    Some(strategy) => strategy,
                    None => return Ok(false),
                },
                binding: pcscope_contracts::identity::SourceRecordRef {
                    source: input.source.clone(),
                    item_id: input.item_id.clone(),
                    native_record_id: if input.evidence.kind == pcscope_contracts::adapter::EvidenceKind::Event {
                        input.evidence.id.clone()
                    } else { input.native_record_id.clone() },
                },
                machine_id: context.machine_id.clone(),
                installation_id: context.installation_id.clone(),
                evidence_time: pcscope_contracts::identity::EvidenceTime {
                    source_time: if input.payload["source_time_available"]==false || action.precision!=TimePrecision::Exact {None}else{input.source_time.clone()},
                    observed_at: action.observed_at.clone()
                },
                hints: pcscope_contracts::identity::LifecycleHints {
                    complete_observation: true,
                    collected_scope: context.collected_scope.clone(),
                    configured: true,
                    history_retained_from: None,
                    states_object_ended: matches!(action.action_id.as_str(), "startup.task_deleted" | "identity.account_deleted" | "files.deleted" | "files.deleted_archived" | "execution.process_stopped"),
                    boot_id: context.boot_id.clone(),
                },
                rule_version: context.rule_version.clone(),
            };
            // The type comes from the target the action resolved to. It used to hold the collection
            // item, which is a different thing and made the column unusable for filtering.
            // Event record IDs can repeat after a log is cleared. The full evidence address
            // includes the epoch, and an event never renames an object through a source binding.
            let resolved=if input.evidence.kind == pcscope_contracts::adapter::EvidenceKind::Event {
                identity::apply_event(tx,&request)?
            } else {identity::apply_expanded(tx,&request)?};
            match resolved.instance_id {
                Some(id) => {crate::display::bind_scope(tx,id,input)?;(Some(id), None, Some(identity.type_id.clone()))},
                None => (None, Some("the target identity could not be resolved".to_owned()), Some(identity.type_id.clone())),
            }
        }
        ActionTarget::Unassigned { reason } => (None, Some(reason.clone()), pcscope_contracts::model::action_definition(&action.action_id).and_then(|value| value["target_type"].as_str().map(str::to_owned))),
    };

    if !claim(tx, "action", &action_key(instance_id, action), &action.evidence)? {
        return Ok(false);
    }
    if let Some(existing) = merged_into(tx, instance_id, action)? {
        for reference in &action.evidence {
            tx.execute(
                "INSERT INTO evidence_refs(subject_kind,subject_id,evidence_kind,evidence_id) VALUES('action',?,?,?) ON CONFLICT DO NOTHING",
                params![existing.to_string(), kind_name(reference), reference.id],
            )?;
        }
        return Ok(false);
    }
    tx.execute(
        "INSERT INTO object_actions(instance_id,unassigned_reason,action_id,target_type,title,result,phase,field_id,before_value,after_value,occurred_at,precision,interval_from,interval_until,observed_at,actor,program,rule_version,occurrence_key,source_record)
         VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
        params![
            instance_id,
            unassigned,
            action.action_id,
            target_type,
            title_of(&action.action_id),
            outcome_name(action.result),
            action.phase,
            action.field_id,
            action.before.as_ref().map(Value::to_string),
            action.after.as_ref().map(Value::to_string),
            action.occurred_at,
            precision_name(action.precision),
            action.interval_from,
            action.interval_until,
            action.observed_at,
            action.actor,
            action.program,
            context.rule_version,
            action.occurrence_key,
            action.source_record.as_ref().map(Value::to_string)
        ],
    )?;
    let row = tx.last_insert_rowid();
    for reference in &action.evidence {
        tx.execute(
            "INSERT INTO evidence_refs(subject_kind,subject_id,evidence_kind,evidence_id) VALUES('action',?,?,?) ON CONFLICT DO NOTHING",
            params![row.to_string(), kind_name(reference), reference.id],
        )?;
    }
    if let Some(instance)=instance_id {
        if let Some(intent)=service_intent.or_else(||pcscope_contracts::event_state::derive_event_state(input,context,action)) {
            for field in &intent.fields {
                record_fact(tx,instance,input,context,field,&action.observed_at,&action.evidence)?;
            }
        }
    }
    Ok(true)
}

/// The identity of one occurrence, used to keep a replay from writing it twice.
fn action_key(instance_id: Option<u64>, action: &ActionCandidate) -> String {
    if let Some(key) = &action.occurrence_key { return key.clone() }
    format!(
        "{}|{}|{}|{}|{}",
        instance_id.map(|id| id.to_string()).unwrap_or_else(|| "-".into()),
        action.action_id,
        action.phase.clone().unwrap_or_default(),
        action.occurred_at.clone().unwrap_or_default(),
        action.interval_until.clone().unwrap_or_else(|| action.observed_at.clone())
    )
}

/// The existing row this action belongs with, when the merge rule says the two are one occurrence.
fn merged_into(tx: &Connection, instance_id: Option<u64>, action: &ActionCandidate) -> Result<Option<i64>> {
    let Some(instance_id) = instance_id else { return Ok(None) };
    let existing: Vec<(i64, String, Option<String>, String, Option<String>, Option<String>, Option<String>, String)> = {
        let mut statement = tx.prepare(
            "SELECT action_row,action_id,phase,result,occurred_at,interval_from,interval_until,observed_at FROM object_actions
             WHERE instance_id=? AND action_id=?",
        )?;
        let rows = statement.query_map(params![instance_id, action.action_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?))
        })?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    for (row, action_id, phase, result, occurred_at, interval_from, interval_until, observed_at) in existing {
        let stored = ActionCandidate {
            occurrence_key: None, source_record: None,
            target: ActionTarget::Assigned { identity: pcscope_contracts::adapter::IdentityCandidate { type_id: String::new(), scope: String::new(), fields: Vec::new(), missing: Vec::new(), ambiguous: None } },
            action_id,
            phase,
            result: outcome_from(&result),
            precision: if occurred_at.is_some() { TimePrecision::Exact } else { TimePrecision::Interval },
            occurred_at,
            interval_from,
            interval_until,
            observed_at,
            field_id: None,
            before: None,
            after: None,
            evidence: Vec::new(),
            actor: None,
            program: None,
        };
        // The stored target is not re-read here: the row was found by instance, which is the target
        // match the rule asks for. What is compared is the occurrence itself.
        if let MergeDecision::Merge { .. } = merge_action(&stored, action) {
            return Ok(Some(row));
        }
    }
    Ok(None)
}

fn outcome_name(outcome: ActionOutcome) -> &'static str {
    use ActionOutcome::*;
    match outcome {
        Succeeded => "succeeded",
        Failed => "failed",
        Started => "started",
        Ended => "ended",
        Blocked => "blocked",
        Observed => "observed",
        Unknown => "unknown",
    }
}

fn outcome_from(name: &str) -> ActionOutcome {
    use ActionOutcome::*;
    match name {
        "succeeded" => Succeeded,
        "failed" => Failed,
        "started" => Started,
        "ended" => Ended,
        "blocked" => Blocked,
        "unknown" => Unknown,
        _ => Observed,
    }
}

fn precision_name(precision: TimePrecision) -> &'static str {
    match precision {
        TimePrecision::Exact => "exact",
        TimePrecision::Interval => "interval",
        TimePrecision::ObservationOnly => "observation_only",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Store;
    use pcscope_contracts::{parse, Envelope};
    use serde_json::{json, Value};

    const TASKS: &str = "pcscope.startup.tasks";
    const TASK_SCOPE: &str = "machine:m/install:i/schema:1/context:/shard:0";
    const TASK_PATH: &str = r"\例子文件夹\例子任务";
    const MACHINE: &str = "m";
    const INSTALL: &str = "i";

    fn network_snapshot(source:&str,run:&str,time:&str,items:Value)->Envelope {
        envelope(json!({"protocol_version":1,"request_id":run,"type":"snapshot","source":source,"scope":"machine:m/schema:1/context:/shard:0","run_id":run,"observed_at":time,"complete":true,"items":items}))
    }
    fn tcp_item(port:u32,state:&str)->Value {
        json!({"id":format!("tcp-{port}"),"data":{"LocalAddress":"127.0.0.1","LocalPort":port,"RemoteAddress":"127.0.0.1","RemotePort":443,"OwningProcess":42,"CreationTime":"2026-10-15T01:00:01Z","State":state}})
    }
    #[test]
    fn snapshot_additions_reach_timeline_without_audit_logs_or_baseline_noise() {
        use pcscope_contracts::query::{ActionFilter,QueryRequest};
        let mut store=Store::open(":memory:").unwrap();let tcp="pcscope.network.tcp";let udp="pcscope.network.udp";
        store.ingest(&network_snapshot(tcp,"baseline","2026-10-15T01:00:00Z",json!([tcp_item(50000,"Listen")]))).unwrap();
        store.ingest(&network_snapshot(udp,"udp-baseline","2026-10-15T01:00:00Z",json!([]))).unwrap();drain(&store);
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions"),0,"initial inventory must not claim new connections");
        let added=network_snapshot(tcp,"added","2026-10-15T01:00:03Z",json!([tcp_item(50000,"Listen"),tcp_item(50001,"Established"),tcp_item(50002,"Listen")]));
        store.ingest(&added).unwrap();
        store.ingest(&network_snapshot(udp,"udp-added","2026-10-15T01:00:03Z",json!([{"id":"udp-5353","data":{"LocalAddress":"0.0.0.0","LocalPort":5353,"OwningProcess":42,"CreationTime":"2026-10-15T01:00:01Z"}}]))).unwrap();drain(&store);
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE action_id='observed.first_seen' AND occurred_at IS NULL AND precision='interval'"),3);
        store.ingest(&added).unwrap();drain(&store);
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE action_id='observed.first_seen'"),3,"replay does not duplicate arrivals");
        store.ingest(&network_snapshot(tcp,"changed","2026-10-15T01:00:04Z",json!([tcp_item(50000,"Listen"),tcp_item(50001,"CloseWait"),tcp_item(50002,"Listen")]))).unwrap();drain(&store);
        let response=store.query(&QueryRequest::ActionTimeline(ActionFilter{type_id:Some("network.endpoint".into()),action_id:Some("observed.first_seen".into()),..Default::default()})).unwrap();
        let rows=response.data.as_ref().unwrap().as_array().unwrap();assert_eq!(rows.len(),3);
        assert!(rows.iter().any(|row|row["source_record"]["snapshot_presence"]["state"]=="Established"));
        assert!(rows.iter().all(|row|row["interval_from"]=="2026-10-15T01:00:00.000Z"&&row["interval_until"]=="2026-10-15T01:00:03Z"));
        store.ingest(&network_snapshot(tcp,"gone","2026-10-15T01:00:05Z",json!([tcp_item(50000,"Listen")]))).unwrap();drain(&store);
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE action_id='observed.no_longer_observed'"),2,"arrivals and disappearances both exist");
    }
    #[test]
    fn snapshot_reappearance_is_a_distinct_idempotent_change_not_a_second_first_sighting() {
        let mut store=Store::open(":memory:").unwrap();let source="pcscope.network.tcp";
        for (run,time,items) in [
            ("base","2026-10-19T01:00:00Z",json!([])),
            ("first","2026-10-19T01:00:01Z",json!([tcp_item(55000,"Listen")])),
            ("gone","2026-10-19T01:00:02Z",json!([])),
            ("again","2026-10-19T01:00:03Z",json!([tcp_item(55000,"Listen")])),
            ("gone-again","2026-10-19T01:00:04Z",json!([])),
        ] {let snapshot=network_snapshot(source,run,time,items);store.ingest(&snapshot).unwrap();drain(&store);store.ingest(&snapshot).unwrap();drain(&store);}
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances"),1);
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE action_id='observed.first_seen'"),1);
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE action_id='observed.reappeared'"),1);
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE action_id='observed.no_longer_observed'"),2);
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE action_id='observed.reappeared' AND precision='interval' AND occurred_at IS NULL AND interval_from='2026-10-19T01:00:02.000Z'"),1);
        store.conn.execute("UPDATE projection_queue SET state='pending'",[]).unwrap();drain(&store);
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions"),4,"historical replay must not manufacture another return");
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances WHERE lifecycle='not_observed'"),1);
    }

    #[test]
    fn parent_membership_changes_have_both_ends_without_parent_arrival_noise() {
        let mut store=Store::open(":memory:").unwrap();let source="pcscope.identity.groups";
        for (index,members) in [json!(["S-1-5-21-100","S-1-5-21-200"]),json!(["S-1-5-21-200","S-1-5-21-300"]),json!([])].into_iter().enumerate() {
            store.ingest(&account_snapshot(source,&format!("members-{index}"),&format!("2026-10-19T01:00:0{index}Z"),json!([{"id":"S-1-5-32-544","data":{"name":"Group","members":members}}]))).unwrap();drain(&store);
        }
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE action_id='observed.first_seen' AND target_type='account.group_membership'"),1);
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE action_id='observed.no_longer_observed' AND target_type='account.group_membership'"),3);
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances WHERE type_id='account.group' AND lifecycle='active'"),1);
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE target_type='account.group'"),0);
        store.conn.execute("UPDATE projection_queue SET state='pending'",[]).unwrap();drain(&store);
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE action_id IN ('observed.first_seen','observed.no_longer_observed','observed.reappeared')"),4);
    }
    #[test]
    fn stable_device_row_rebinds_driver_a_b_a_without_merging_versions() {
        let mut store=Store::open(":memory:").unwrap();let source="pcscope.hardware.driver-bindings";
        for (index,version) in ["1.0","2.0","1.0"].into_iter().enumerate(){
            store.ingest(&network_snapshot(source,&format!("driver-{index}"),&format!("2026-10-19T01:00:0{index}Z"),json!([{"id":"device-1","data":{"DeviceID":"device-1","DeviceName":"Example device","DriverVersion":version}}]))).unwrap();drain(&store);
        }
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances WHERE type_id='hardware.driver_binding'"),2);
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances WHERE type_id='hardware.driver_binding' AND lifecycle='active'"),1);
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE action_id='observed.no_longer_observed'"),2);
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE action_id='observed.first_seen'"),1);
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE action_id='observed.reappeared'"),1);
        assert_eq!(count(&store,"SELECT count(*) FROM source_bindings WHERE valid_to IS NULL"),1);
    }
    #[test]
    fn registry_value_children_have_arrivals_absences_and_reject_unreadable_tables() {
        let mut store=Store::open(":memory:").unwrap();let source="pcscope.environment.configuration";
        let values=[json!({"A":{"kind":"String","value":"one"},"B":{"kind":"String","value":"two"}}),
            json!({"B":{"kind":"String","value":"two"},"C":{"kind":"String","value":"three"}}),json!("unreadable"),json!({})];
        for (index,values) in values.into_iter().enumerate(){
            store.ingest(&network_snapshot(source,&format!("env-{index}"),&format!("2026-10-19T01:00:0{index}Z"),json!([{"id":"machine-env","data":{"hive":"LocalMachine","view":"Registry64","path":"SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment","values":values}}]))).unwrap();drain(&store);
            if index==2{assert_eq!(count(&store,"SELECT count(*) FROM object_instances WHERE lifecycle='active'"),2,"invalid table cannot mean empty");}
        }
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances WHERE type_id='policy.environment_variable'"),3);
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances WHERE type_id='policy.environment_variable' AND lifecycle='not_observed'"),3);
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE action_id='observed.first_seen'"),1);
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE action_id='observed.no_longer_observed'"),3);
    }
    #[test]
    fn complete_registry_key_sets_reconcile_browser_uac_and_logon_entries() {
        for (source,path,typ) in [
            ("pcscope.apps.browser-policies",r"Software\Policies\Google\Chrome","policy.browser_policy"),
            ("pcscope.policy.configuration",r"Software\Microsoft\Windows\CurrentVersion\Policies\System","policy.uac_policy"),
            ("pcscope.startup.logon",r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run","startup.logon_startup_entry"),
        ]{
            let mut store=Store::open(":memory:").unwrap();
            for (index,values) in [json!({"A":{"kind":"String","value":"one"},"B":{"kind":"String","value":"two"}}),json!({"B":{"kind":"String","value":"two"},"C":{"kind":"String","value":"three"}}),json!({})].into_iter().enumerate(){
                let item=if source=="pcscope.startup.logon"{json!({"id":"LocalMachine/Registry64/registry-key","data":{"registry_path":path,"values":values}})}
                    else{json!({"id":"registry-key","data":{"hive":"LocalMachine","view":"Registry64","path":path,"values":values}})};
                store.ingest(&network_snapshot(source,&format!("key-{index}"),&format!("2026-10-19T01:00:0{index}Z"),json!([item]))).unwrap();drain(&store);
            }
            assert_eq!(store.conn.query_row("SELECT count(*) FROM object_instances WHERE type_id=? AND lifecycle='not_observed'",[typ],|r|r.get::<_,i64>(0)).unwrap(),3,"{source}");
            assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE action_id='observed.first_seen'"),1,"{source}");
            assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE action_id='observed.no_longer_observed'"),3,"{source}");
        }
    }
    #[test]
    fn exclusion_and_file_stream_sets_require_explicit_complete_arrays() {
        for (source,typ) in [("pcscope.security.protection-policy","security.exclusion"),("pcscope.files.metadata-security","file.alternate_data_stream")]{
            let mut store=Store::open(":memory:").unwrap();
            for (index,members) in [json!(["A","B"]),json!(["B","C"]),Value::Null,json!([])].into_iter().enumerate(){
                let payload=if typ=="security.exclusion"{json!({"ExclusionPath":members,"ExclusionProcess":[],"ExclusionExtension":[],"DisableRealtimeMonitoring":false})}
                    else{json!({"exists":true,"volume_identity":"volume-1","file_id":"file-1","path":r"C:\Example\file.txt","streams":members.as_array().map(|values|values.iter().map(|v|json!({"name":format!(":{}",v.as_str().unwrap()),"size":1})).collect::<Vec<_>>())})};
                store.ingest(&network_snapshot(source,&format!("array-{index}"),&format!("2026-10-19T01:00:0{index}Z"),json!([{"id":"parent","data":payload}]))).unwrap();drain(&store);
                if index==2{assert_eq!(store.conn.query_row("SELECT count(*) FROM object_instances WHERE type_id=? AND lifecycle='active'",[typ],|r|r.get::<_,i64>(0)).unwrap(),2,"unreadable list cannot clear {typ}");}
            }
            assert_eq!(store.conn.query_row("SELECT count(*) FROM object_actions WHERE target_type=? AND action_id='observed.first_seen'",[typ],|r|r.get::<_,i64>(0)).unwrap(),1,"{typ}");
            assert_eq!(store.conn.query_row("SELECT count(*) FROM object_actions WHERE target_type=? AND action_id='observed.no_longer_observed'",[typ],|r|r.get::<_,i64>(0)).unwrap(),3,"{typ}");
        }
    }
    #[test]
    fn removing_one_source_does_not_retire_an_object_still_reported_by_another() {
        let mut store=Store::open(":memory:").unwrap();let source="pcscope.storage.disks";let health="pcscope.hardware.disk-health";
        let item=json!([{"id":"disk-1","data":{"ObjectId":"disk-1","FriendlyName":"Example disk","HealthStatus":"Healthy"}}]);
        for (src,run) in [(source,"disk-base"),(health,"health-base")] {store.ingest(&network_snapshot(src,run,"2026-10-19T01:00:00Z",item.clone())).unwrap();drain(&store);}
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances"),1);
        store.ingest(&network_snapshot(source,"disk-gone","2026-10-19T01:00:01Z",json!([]))).unwrap();drain(&store);
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances WHERE lifecycle='active'"),1);
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE action_id='observed.no_longer_observed'"),0);
        store.ingest(&network_snapshot(health,"health-gone","2026-10-19T01:00:02Z",json!([]))).unwrap();drain(&store);
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances WHERE lifecycle='not_observed'"),1);
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE action_id='observed.no_longer_observed'"),1);
    }

    #[test]
    fn native_sysmon_family_reaches_stored_actions_and_readable_query_subjects() {
        use pcscope_contracts::query::{ActionFilter,QueryRequest};
        let mut store=Store::open(":memory:").unwrap();let source="pcscope.eventlog.microsoft-windows-sysmon/operational";
        let events:Vec<Value>=[(1,5),(5,3),(6,4),(7,3),(3,5),(11,2),(23,5),(26,5)].into_iter().map(|(id,version)|json!({
            "id":id.to_string(),"kind":"execution.sysmon_event","object_id":format!("Microsoft-Windows-Sysmon/Operational/Microsoft-Windows-Sysmon/{id}"),"occurred_at":format!("2026-10-19T01:00:{id:02}Z"),
            "data":{"channel":"Microsoft-Windows-Sysmon/Operational","provider":"Microsoft-Windows-Sysmon","event_id":id,"record_id":id,"system":{"Version":{"value":version.to_string()}},"source_time_available":true,
                "fields":{"ProcessGuid":"{01234567-1234-1234-1234-123456789abc}","ProcessId":"42","Image":"C:\\Example\\example.exe","ImageLoaded":"C:\\Example\\module.dll","TargetFilename":"C:\\Example\\file.txt","Protocol":"tcp","SourceIp":"127.0.0.1","SourcePort":"55000","DestinationIp":"127.0.0.1","DestinationPort":"443"}}})).collect();
        let batch=envelope(json!({"protocol_version":1,"request_id":"sysmon-native","type":"event_batch","source":source,"epoch":"first","batch_seq":1,"checkpoint":"26","events":events}));
        store.ingest(&batch).unwrap();drain(&store);store.ingest(&batch).unwrap();drain(&store);
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions"),8);
        assert_eq!(count(&store,"SELECT count(*) FROM projection_queue WHERE state='failed'"),0);
        let rows=store.query(&QueryRequest::ActionTimeline(ActionFilter{..Default::default()})).unwrap().data.unwrap();
        for row in rows.as_array().unwrap(){assert_ne!(row["context"]["classification_status"],"unverified");
            if row["action_id"]=="files.created" {assert_eq!(row["context"]["subject_label"],"file.txt");}
            if row["action_id"]=="execution.driver_loaded" {assert_eq!(row["context"]["subject_label"],"module.dll");assert!(row["instance_id"].is_null(),"image path is not a service identity");}
        }
    }
    #[test]
    fn retained_addition_recovery_is_bounded_idempotent_and_preserves_raw_material() {
        let mut store=Store::open(":memory:").unwrap();let source="pcscope.network.tcp";
        store.ingest(&network_snapshot(source,"baseline","2026-10-15T01:00:00Z",json!([]))).unwrap();drain(&store);
        store.ingest(&network_snapshot(source,"added","2026-10-15T01:00:03Z",json!([tcp_item(50001,"Established")]))).unwrap();
        store.conn.execute("DELETE FROM projection_queue WHERE evidence_id LIKE 'snapshot-appearance:%'",[]).unwrap();
        store.conn.execute("DELETE FROM evidence WHERE evidence_id LIKE 'snapshot-appearance:%'",[]).unwrap();drain(&store);
        let text:String=store.conn.query_row("SELECT data FROM records WHERE kind='object.first_seen'",[],|row|row.get(0)).unwrap();
        let mut legacy:Value=serde_json::from_str(&text).unwrap();legacy.as_object_mut().unwrap().remove("_snapshot_appearance_v1");
        store.conn.execute("UPDATE records SET data=? WHERE kind='object.first_seen'",[legacy.to_string()]).unwrap();
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions"),0);
        let before:String=store.conn.query_row("SELECT data FROM records WHERE kind='object.first_seen'",[],|row|row.get(0)).unwrap();
        assert_eq!(store.repair_snapshot_appearance(1).unwrap()["queued"],1);drain(&store);
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE action_id='observed.first_seen'"),1);
        assert_eq!(store.repair_snapshot_appearance(1).unwrap()["queued"],0);
        let after:String=store.conn.query_row("SELECT data FROM records WHERE kind='object.first_seen'",[],|row|row.get(0)).unwrap();assert_eq!(before,after);
        assert!(store.repair_snapshot_appearance(0).is_err());assert!(store.repair_snapshot_appearance(257).is_err());
    }
    #[test]
    fn a_new_source_slice_for_a_known_object_does_not_create_an_arrival() {
        let mut store=Store::open(":memory:").unwrap();
        store.ingest(&tasks_snapshot("known","2026-10-15T01:00:00Z","Known task",true)).unwrap();drain(&store);
        let mut empty=tasks_snapshot("other-base","2026-10-15T01:00:01Z","Known task",true);
        if let pcscope_contracts::Message::Snapshot{scope,items,..}=&mut empty.message {*scope="machine:m/install:i/schema:1/context:other/shard:0".into();items.clear();}
        store.ingest(&empty).unwrap();
        let mut extra=tasks_snapshot("other-role","2026-10-15T01:00:03Z","Known task",true);
        if let pcscope_contracts::Message::Snapshot{scope,..}=&mut extra.message {*scope="machine:m/install:i/schema:1/context:other/shard:0".into();}
        store.ingest(&extra).unwrap();drain(&store);
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE action_id='observed.first_seen'"),0);
    }
    #[test]
    fn missing_addition_material_does_not_block_other_pending_observations() {
        let mut store=Store::open(":memory:").unwrap();let source="pcscope.network.tcp";
        store.ingest(&network_snapshot(source,"baseline","2026-10-15T01:00:00Z",json!([]))).unwrap();drain(&store);
        store.ingest(&network_snapshot(source,"added","2026-10-15T01:00:03Z",json!([tcp_item(50001,"Established"),tcp_item(50002,"Listen")]))).unwrap();
        store.conn.execute("DELETE FROM records WHERE object_id='tcp-50001'",[]).unwrap();
        let report=store.project_pending(MACHINE,INSTALL,100).unwrap();assert_eq!(report.rejected,1);
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE action_id='observed.first_seen'"),1);
        assert_eq!(count(&store,"SELECT count(*) FROM projection_queue WHERE state='pending'"),0,"a missing historical record must not cause a retry loop");
    }

    fn envelope(value: Value) -> Envelope {
        parse(&value.to_string()).unwrap()
    }
    #[test]
    fn legacy_absence_repair_retires_a_stale_object_without_logs() {
        let mut store=Store::open(":memory:").unwrap();
        store.ingest(&tasks_snapshot("present","2026-09-12T01:00:00Z","例子任务",true)).unwrap();drain(&store);
        let absent=envelope(json!({"protocol_version":1,"request_id":"absent","type":"snapshot","source":TASKS,"scope":TASK_SCOPE,"run_id":"absent","observed_at":"2026-09-12T01:00:03Z","complete":true,"items":[]}));
        store.ingest(&absent).unwrap();
        // Simulate pre-fix ingestion only in this isolated fixture: raw difference exists, but
        // it was never queued for projection and has no new-format marker.
        store.conn.execute("DELETE FROM projection_queue WHERE evidence_id LIKE 'snapshot-absence:%'",[]).unwrap();
        store.conn.execute("DELETE FROM evidence WHERE evidence_id LIKE 'snapshot-absence:%'",[]).unwrap();
        let(seq,text):(i64,String)=store.conn.query_row("SELECT seq,data FROM records WHERE kind='object.no_longer_observed'",[],|row|Ok((row.get(0)?,row.get(1)?))).unwrap();
        let mut data:Value=serde_json::from_str(&text).unwrap();data.as_object_mut().unwrap().remove("_snapshot_absence_v1");
        let original=data.to_string();
        store.conn.execute("UPDATE records SET data=? WHERE seq=?",rusqlite::params![original,seq]).unwrap();
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances WHERE lifecycle='active'"),1);
        assert_eq!(store.repair_snapshot_absence(256).unwrap()["queued"],1);drain(&store);
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances WHERE lifecycle='not_observed'"),1);
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE action_id='observed.no_longer_observed' AND precision='interval' AND occurred_at IS NULL"),1);
        let retained:String=store.conn.query_row("SELECT data FROM records WHERE seq=?",[seq],|row|row.get(0)).unwrap();assert_eq!(retained,original);
        assert_eq!(store.repair_snapshot_absence(256).unwrap()["queued"],0);
    }
    #[test]
    fn complete_snapshot_absence_updates_object_and_timeline_without_logs_and_replays_once() {
        let mut store=Store::open(":memory:").unwrap();
        store.ingest(&tasks_snapshot("present","2026-09-12T01:00:00Z","例子任务",true)).unwrap();drain(&store);
        let partial=envelope(json!({"protocol_version":1,"request_id":"partial","type":"snapshot","source":TASKS,"scope":TASK_SCOPE,"run_id":"partial","observed_at":"2026-09-12T01:00:01Z","complete":false,"items":[]}));
        assert!(store.ingest(&partial).is_err());
        let gap=envelope(json!({"protocol_version":1,"request_id":"gap","type":"gap","source":TASKS,"observed_at":"2026-09-12T01:00:02Z","reason":"unavailable"}));
        store.ingest(&gap).unwrap();drain(&store);assert_eq!(count(&store,"SELECT count(*) FROM object_instances WHERE lifecycle='active'"),1);
        let absent=envelope(json!({"protocol_version":1,"request_id":"absent","type":"snapshot","source":TASKS,"scope":TASK_SCOPE,"run_id":"absent","observed_at":"2026-09-12T01:00:03Z","complete":true,"items":[]}));
        store.ingest(&absent).unwrap();drain(&store);
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances WHERE lifecycle='not_observed' AND last_observed_at='2026-09-12T01:00:00Z' AND last_absent_at='2026-09-12T01:00:03Z'"),1);
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE action_id='observed.no_longer_observed' AND precision='interval' AND interval_from='2026-09-12T01:00:00Z' AND interval_until='2026-09-12T01:00:03Z' AND occurred_at IS NULL AND actor IS NULL AND program IS NULL"),1);
        assert_eq!(count(&store,"SELECT count(*) FROM evidence_refs WHERE subject_kind='action' AND evidence_kind='record'"),1);
        store.ingest(&absent).unwrap();drain(&store);assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE action_id='observed.no_longer_observed'"),1);
        store.ingest(&tasks_snapshot("returned","2026-09-12T01:00:04Z","例子任务",true)).unwrap();drain(&store);
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances WHERE lifecycle='active'"),1);
    }
    #[test]
    fn delayed_snapshot_absence_cannot_overwrite_a_newer_presence_in_another_collected_slice() {
        let mut store=Store::open(":memory:").unwrap();
        store.ingest(&tasks_snapshot("present","2026-09-12T01:00:00Z","例子任务",true)).unwrap();drain(&store);
        let mut newer=tasks_snapshot("newer","2026-09-12T01:00:04Z","例子任务",true);
        if let pcscope_contracts::Message::Snapshot{scope,..}=&mut newer.message {*scope="machine:m/install:i/schema:1/context:other/shard:0".into();}
        store.ingest(&newer).unwrap();drain(&store);
        let absent=envelope(json!({"protocol_version":1,"request_id":"late-absent","type":"snapshot","source":TASKS,"scope":TASK_SCOPE,"run_id":"late-absent","observed_at":"2026-09-12T01:00:03Z","complete":true,"items":[]}));
        store.ingest(&absent).unwrap();drain(&store);
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances WHERE lifecycle='active' AND last_observed_at='2026-09-12T01:00:04Z'"),1);
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE action_id='observed.no_longer_observed'"),0);
    }

    fn tasks_snapshot(run: &str, observed_at: &str, name: &str, enabled: bool) -> Envelope {
        envelope(json!({
            "protocol_version": 1, "request_id": run, "type": "snapshot",
            "source": TASKS, "scope": TASK_SCOPE, "run_id": run, "observed_at": observed_at, "complete": true,
            "items": [{"id": TASK_PATH, "data": {"path": TASK_PATH, "name": name, "enabled": enabled, "principal": "示例用户"}}]
        }))
    }

    /// One batch carrying both stages of a task run: the start and the end.
    fn task_run_events() -> Envelope {
        let event = |id: &str, event_id: i64, occurred_at: &str| json!({
            "id": id, "occurred_at": occurred_at, "kind": "windows.event", "object_id": id,
            "data": {"provider": "Microsoft-Windows-TaskScheduler", "event_id": event_id, "task_name": TASK_PATH}
        });
        envelope(json!({
            "protocol_version": 1, "request_id": "batch-1", "type": "event_batch",
            "source": "pcscope.eventlog.microsoft-windows-taskscheduler/operational",
            "epoch": "1", "batch_seq": 1, "checkpoint": "cp-1",
            "events": [event("100", 100, "2026-09-12T08:00:00Z"), event("102", 102, "2026-09-12T08:00:05Z")]
        }))
    }

    fn count(store: &Store, sql: &str) -> i64 {
        store.conn.query_row(sql, [], |r| r.get(0)).unwrap()
    }

    fn pending(store: &Store) -> i64 {
        count(store, "SELECT count(*) FROM projection_queue WHERE state='pending'")
    }

    fn drain(store: &Store) -> Report {
        store.project_pending(MACHINE, INSTALL, 100).unwrap()
    }

    fn fact(store: &Store, instance_id: u64, field_id: &str) -> Option<(String, String)> {
        store
            .conn
            .query_row("SELECT value,value_status FROM object_facts WHERE instance_id=? AND field_id=?", rusqlite::params![instance_id, field_id], |r| Ok((r.get(0)?, r.get(1)?)))
            .ok()
    }

    fn account_snapshot(source: &str, run: &str, time: &str, items: Value) -> Envelope {
        envelope(json!({
            "protocol_version":1,"request_id":run,"type":"snapshot","source":source,
            "scope":TASK_SCOPE,"run_id":run,"observed_at":time,"complete":true,"items":items
        }))
    }

    fn instance_for_key(store: &Store, type_id: &str, suffix: &str) -> u64 {
        store.conn.query_row("SELECT instance_id FROM object_instances WHERE type_id=? AND native_key LIKE ?", params![type_id,format!("%{suffix}")], |r| r.get(0)).unwrap()
    }

    fn explicit_event(channel:&str,provider:&str,event_id:u64,fields:Value,epoch:&str,time:&str,imported:bool)->Envelope {
        let source=if imported {"pcscope.eventlog.import.fixture".into()}else{format!("pcscope.eventlog.{}",channel.to_lowercase())};
        envelope(json!({"protocol_version":1,"request_id":epoch,"type":"event_batch","source":source,
            "epoch":epoch,"batch_seq":1,"checkpoint":"1","events":[{"id":"1","occurred_at":time,
            "kind":"windows.event","object_id":"1","data":{"channel":channel,"provider":provider,"event_id":event_id,
            "fields":fields,"system":{"Version":{"value":"0"}},"source_time_available":true}}]}))
    }

    #[test]
    fn explicit_account_events_update_state_with_time_order_epoch_and_import_isolation() {
        let mut store=Store::open(":memory:").unwrap();
        let mut snapshot=account_snapshot("pcscope.identity.accounts","user","2026-10-13T01:00:00Z",
            json!([{"id":"S-1-5-21-100","data":{"name":"Example","enabled":true}}]));
        if let pcscope_contracts::Message::Snapshot {scope,..}=&mut snapshot.message {*scope="machine:m".into();}
        store.ingest(&snapshot).unwrap();drain(&store);
        for (sid,id,epoch,time,imported) in [
            ("S-1-5-21-100",4725,"disable","2026-10-13T02:00:00Z",false),
            ("S-1-5-21-100",4722,"late","2026-10-13T01:30:00Z",false),
            ("S-1-5-21-200",4722,"new-epoch","2026-10-13T03:00:00Z",false),
            ("S-1-5-21-100",4722,"import","2026-10-13T04:00:00Z",true),
        ] {
            store.ingest(&explicit_event("Security","Microsoft-Windows-Security-Auditing",id,json!({"TargetSid":sid}),epoch,time,imported)).unwrap();
            assert_eq!(drain(&store).rejected,0);
        }
        let first=instance_for_key(&store,"account.user","s-1-5-21-100");
        let second=instance_for_key(&store,"account.user","s-1-5-21-200");
        assert_ne!(first,second);assert_eq!(fact(&store,first,"enabled").unwrap().0,"false");
        assert_eq!(fact(&store,second,"enabled").unwrap().0,"true");
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions"),4,"no duplicate synthetic state event");
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE instance_id IS NULL"),1);
        store.conn.execute("UPDATE projection_queue SET state='pending'",[]).unwrap();drain(&store);
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions"),4);
        assert_eq!(fact(&store,first,"enabled").unwrap().0,"false");
    }

    #[test]
    fn scm_state_needs_one_matching_service_and_is_readable_in_timeline() {
        for duplicate in [false,true] {
            let mut store=Store::open(":memory:").unwrap();
            let mut items=vec![json!({"id":"svc-one","data":{"name":"svc-one","display_name":"Example service","state":"Running"}})];
            if duplicate {items.push(json!({"id":"svc-two","data":{"name":"svc-two","display_name":"Example service","state":"Running"}}));}
            let mut snapshot=account_snapshot("pcscope.startup.services","services","2026-10-13T01:00:00Z",json!(items));
            if let pcscope_contracts::Message::Snapshot {scope,..}=&mut snapshot.message {*scope="machine:m".into();}
            store.ingest(&snapshot).unwrap();drain(&store);
            store.ingest(&explicit_event("System","Service Control Manager",7036,json!({"param1":"Example service","param2":"stopped"}),"scm","2026-10-13T02:00:00Z",false)).unwrap();
            assert_eq!(drain(&store).rejected,0);
            let id=instance_for_key(&store,"startup.service","svc-one");
            assert_eq!(fact(&store,id,"state").unwrap().0,json!(if duplicate {"Running"}else{"Stopped"}).to_string());
            let timeline=store.query(&pcscope_contracts::query::QueryRequest::ActionTimeline(Default::default())).unwrap().data.unwrap();
            assert_eq!(timeline.as_array().unwrap().len(),1);
            assert_eq!(timeline[0]["context"]["attribution_status"],if duplicate {"unassigned"}else{"verified"});
            assert_eq!(timeline[0]["context"]["subject_label"],"Example service");
        }
    }

    #[test]
    fn complementary_snapshot_does_not_erase_an_earlier_known_field() {
        let mut store=Store::open(":memory:").unwrap();
        store.ingest(&tasks_snapshot("detail","2026-10-13T01:00:00Z","Example",true)).unwrap();drain(&store);
        let mut inventory=tasks_snapshot("inventory","2026-10-13T02:00:00Z","Example",true);
        if let pcscope_contracts::Message::Snapshot {items,..}=&mut inventory.message {items[0].data.as_object_mut().unwrap().remove("principal");}
        store.ingest(&inventory).unwrap();drain(&store);
        assert_eq!(fact(&store,1,"principal").unwrap(),(json!("示例用户").to_string(),"known".into()));
        assert_eq!(store.conn.query_row("SELECT observed_at FROM object_facts WHERE instance_id=1 AND field_id='principal'",[],|r|r.get::<_,String>(0)).unwrap(),"2026-10-13T01:00:00Z");
    }

    #[test]
    fn equal_time_conflict_survives_repeating_either_value_until_newer_evidence() {
        let mut store=Store::open(":memory:").unwrap();
        let mut snapshot=account_snapshot("pcscope.identity.accounts","base","2026-10-13T01:00:00Z",
            json!([{"id":"S-1-5-21-100","data":{"name":"Example","enabled":true}}]));
        if let pcscope_contracts::Message::Snapshot {scope,..}=&mut snapshot.message {*scope="machine:m".into();}
        store.ingest(&snapshot).unwrap();drain(&store);
        for (event_id,epoch) in [(4725,"conflict"),(4722,"repeat")] {
            store.ingest(&explicit_event("Security","Microsoft-Windows-Security-Auditing",event_id,json!({"TargetSid":"S-1-5-21-100"}),epoch,"2026-10-13T01:00:00Z",false)).unwrap();
            drain(&store);
            assert_eq!(fact(&store,1,"enabled").unwrap().1,"conflicting");
        }
        store.ingest(&explicit_event("Security","Microsoft-Windows-Security-Auditing",4722,json!({"TargetSid":"S-1-5-21-100"}),"new","2026-10-13T02:00:00Z",false)).unwrap();
        drain(&store);assert_eq!(fact(&store,1,"enabled").unwrap(),("true".into(),"known".into()));
    }

    #[test]
    fn expanded_runtime_versions_do_not_merge_when_one_child_remains() {
        let mut store=Store::open(":memory:").unwrap();
        let source="pcscope.apps.dotnet-runtimes";
        let items=|values:Value|json!([{"id":"runtime-parent","data":{
            "path":"SOFTWARE\\dotnet\\Setup\\InstalledVersions\\x64\\sharedfx\\Microsoft.NETCore.App",
            "view":"Registry64","values":values
        }}]);
        for (run,time,values) in [
            ("one","2026-10-13T01:00:00Z",json!({"8.0.1":{"value":1},"9.0.2":{"value":1}})),
            ("two","2026-10-13T02:00:00Z",json!({"9.0.2":{"value":1}})),
        ] {
            store.ingest(&account_snapshot(source,run,time,items(values))).unwrap();
            assert_eq!(drain(&store).rejected,0);
        }
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances WHERE type_id='software.runtime'"),2);
        assert_eq!(count(&store,"SELECT count(DISTINCT value) FROM object_facts WHERE field_id='version'"),2);
    }

    #[test]
    fn late_snapshot_cannot_replace_name_or_create_backwards_change() {
        let mut store=Store::open(":memory:").unwrap();
        store.ingest(&tasks_snapshot("new","2026-10-13T02:00:00Z","New name",true)).unwrap();
        drain(&store);
        store.ingest(&tasks_snapshot("old","2026-10-13T01:00:00Z","Old name",false)).unwrap();
        drain(&store);
        assert_eq!(store.conn.query_row("SELECT display_name FROM object_instances",[],|r|r.get::<_,String>(0)).unwrap(),"New name");
        assert_eq!(fact(&store,1,"enabled").unwrap().0,"true");
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE action_id='observed.configuration_changed'"),0);
        assert_eq!(count(&store,"SELECT count(*) FROM fact_history WHERE observed_from > observed_until"),0);
    }

    #[test]
    fn expanded_rights_keep_each_assignment_and_only_mark_missing_children_absent() {
        let mut store = Store::open(":memory:").unwrap();
        let source = "pcscope.identity.assigned-rights";
        let items = |rights: Value| json!([
            {"id":"S-1-5-21-100","data":{"sid":"S-1-5-21-100","rights":rights}},
            {"id":"S-1-5-21-200","data":{"sid":"S-1-5-21-200","rights":["SeBackupPrivilege"]}}
        ]);
        store.ingest(&account_snapshot(source,"rights-1","2026-10-13T01:00:00Z",items(json!(["SeBackupPrivilege","SeRestorePrivilege"])))).unwrap();
        drain(&store);
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances WHERE type_id='account.right_assignment'"),3);
        let ids: Vec<(u64,String)> = {
            let mut q=store.conn.prepare("SELECT i.instance_id,f.value FROM object_instances i JOIN object_facts f ON f.instance_id=i.instance_id WHERE f.field_id='privilege' ORDER BY i.instance_id").unwrap();
            q.query_map([],|r|Ok((r.get(0)?,r.get(1)?))).unwrap().collect::<rusqlite::Result<_>>().unwrap()
        };
        assert_eq!(ids.len(),3);
        assert_eq!(ids[0].1,json!("SeBackupPrivilege").to_string());
        assert_eq!(ids[1].1,json!("SeRestorePrivilege").to_string());
        store.ingest(&account_snapshot(source,"rights-2","2026-10-13T02:00:00Z",items(json!(["SeRestorePrivilege","SeBackupPrivilege"])))).unwrap();
        drain(&store);
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances"),3,"list order and resampling cannot change identity");
        store.ingest(&account_snapshot(source,"rights-3","2026-10-13T03:00:00Z",items(json!(["SeRestorePrivilege"])))).unwrap();
        drain(&store);
        assert_eq!(store.instance_summary(ids[0].0).unwrap().unwrap()["lifecycle"],"not_observed");
        for id in [ids[1].0,ids[2].0] {assert_eq!(store.instance_summary(id).unwrap().unwrap()["lifecycle"],"active");}
        assert_eq!(fact(&store,ids[0].0,"privilege").unwrap().0,json!("SeBackupPrivilege").to_string(),"last observed fact is retained");
        assert_eq!(count(&store,"SELECT count(*) FROM evidence_refs WHERE subject_kind='observation'"),9,"each current or absent child cites the complete parent list");
        store.conn.execute("UPDATE projection_queue SET state='pending'",[]).unwrap();
        drain(&store);
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances"),3);
        assert_eq!(count(&store,"SELECT count(*) FROM source_bindings"),3);
        assert_eq!(count(&store,"SELECT count(*) FROM evidence_refs WHERE subject_kind='observation'"),9);
        assert_eq!(store.instance_summary(ids[0].0).unwrap().unwrap()["lifecycle"],"not_observed","replaying older presence does not undo a later complete absence");
    }

    #[test]
    fn expanded_groups_keep_the_group_and_each_membership_separate() {
        let mut store = Store::open(":memory:").unwrap();
        let source="pcscope.identity.groups";
        let items=|name:&str,members:Value|json!([
            {"id":"S-1-5-32-544","data":{"name":name,"members":members}},
            {"id":"S-1-5-32-545","data":{"name":"Other group","members":["S-1-5-21-300"]}}
        ]);
        store.ingest(&account_snapshot(source,"groups-1","2026-10-13T01:00:00Z",items("Group",json!(["S-1-5-21-100","S-1-5-21-200"])))).unwrap();
        drain(&store);
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances WHERE type_id='account.group'"),2);
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances WHERE type_id='account.group_membership'"),3);
        let group=instance_for_key(&store,"account.group","s-1-5-32-544");
        let first=instance_for_key(&store,"account.group_membership","s-1-5-21-100");
        let second=instance_for_key(&store,"account.group_membership","s-1-5-21-200");
        let other=instance_for_key(&store,"account.group_membership","s-1-5-21-300");
        store.ingest(&account_snapshot(source,"groups-2","2026-10-13T02:00:00Z",items("Renamed group",json!(["S-1-5-21-200","S-1-5-21-100"])))).unwrap();
        drain(&store);
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances"),5);
        assert_eq!(fact(&store,group,"name").unwrap().0,json!("Renamed group").to_string());
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE field_id='members'"),0,"member order alone cannot be a change");
        store.ingest(&account_snapshot(source,"groups-3","2026-10-13T03:00:00Z",items("Renamed group",json!(["S-1-5-21-200"])))).unwrap();
        drain(&store);
        assert_eq!(store.instance_summary(first).unwrap().unwrap()["lifecycle"],"not_observed");
        for id in [group,second,other] {assert_eq!(store.instance_summary(id).unwrap().unwrap()["lifecycle"],"active");}
        assert_eq!(count(&store,"SELECT count(*) FROM source_bindings"),5);
        assert_eq!(count(&store,"SELECT count(*) FROM evidence_refs WHERE subject_kind='observation'"),15);
    }

    #[test]
    fn expanded_legacy_parent_bindings_recover_on_the_next_snapshot_without_rewriting_history() {
        for (source,initial,expected) in [
            ("pcscope.identity.assigned-rights",json!({"id":"S-1-5-21-100","data":{"rights":["SeBackupPrivilege","SeRestorePrivilege"]}}),2),
            ("pcscope.identity.groups",json!({"id":"S-1-5-32-544","data":{"name":"Group","members":["S-1-5-21-100","S-1-5-21-200"]}}),3),
        ] {
            let mut store=Store::open(":memory:").unwrap();
            store.ingest(&account_snapshot(source,"legacy","2026-10-13T01:00:00Z",json!([initial.clone()]))).unwrap();
            let input=store.evidence_inputs(0,10).unwrap().remove(0);
            let context=AdapterContext {
                catalog_version:pcscope_contracts::model::catalog_version().into(),
                rule_version:pcscope_contracts::model::catalog_fingerprint(),machine_id:MACHINE.into(),installation_id:INSTALL.into(),
                boot_id:None,collected_scope:business_scope(TASK_SCOPE),complete:true,
            };
            // The old projection retained only the first candidate and its parent-record binding,
            // while the original snapshot still contained every child. Seed exactly that state.
            let mut result=adapter_for(source).unwrap().adapt(&input,&context);
            result.observations.truncate(1);
            apply(&store.conn,&input,&context,&result,&Default::default()).unwrap();
            store.conn.execute("UPDATE projection_queue SET state='done'",[]).unwrap();
            let old_key:String=store.conn.query_row("SELECT native_key FROM object_instances",[],|r|r.get(0)).unwrap();
            assert_eq!(count(&store,"SELECT count(*) FROM source_bindings"),1);
            store.ingest(&account_snapshot(source,"recovered","2026-10-13T02:00:00Z",json!([initial]))).unwrap();
            drain(&store);
            assert_eq!(count(&store,"SELECT count(*) FROM object_instances"),expected);
            assert_eq!(count(&store,"SELECT count(*) FROM source_bindings"),expected);
            assert_eq!(store.conn.query_row("SELECT native_key FROM object_instances WHERE instance_id=1",[],|r|r.get::<_,String>(0)).unwrap(),old_key);
            assert_eq!(count(&store,"SELECT count(*) FROM snapshots"),2,"both raw snapshots stay intact");
            assert_eq!(store.conn.query_row("SELECT count(*) FROM evidence_refs WHERE subject_kind='observation' AND evidence_id=?",[&input.evidence.id],|r|r.get::<_,i64>(0)).unwrap(),1,"legacy evidence is not rewritten to invent past observations");
            assert_eq!(count(&store,"SELECT count(*) FROM evidence_refs WHERE subject_kind='observation'"),expected+1);
        }
    }

    #[test]
    fn expanded_absence_requires_an_enabled_readable_complete_parent_list() {
        let mut store=Store::open(":memory:").unwrap();
        let source="pcscope.identity.assigned-rights";
        let items=|rights:Value|json!([{"id":"S-1-5-21-100","data":{"rights":rights}}]);
        store.ingest(&account_snapshot(source,"guard-1","2026-10-13T01:00:00Z",items(json!(["SeBackupPrivilege","SeRestorePrivilege"])))).unwrap();
        drain(&store);
        store.ingest(&account_snapshot(source,"guard-2","2026-10-13T02:00:00Z",items(json!([])))).unwrap();
        let item=crate::item_controls::classify(source,"S-1-5-21-100","",&Value::Null);
        store.conn.execute("INSERT OR REPLACE INTO settings VALUES('item_settings',?)",[json!({"disabled":[item]}).to_string()]).unwrap();
        drain(&store);
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances WHERE lifecycle='active'"),2,"disabling a collection is not absence");
        store.conn.execute("DELETE FROM settings WHERE key='item_settings'",[]).unwrap();
        store.ingest(&account_snapshot(source,"guard-3","2026-10-13T03:00:00Z",items(json!(["SeRestorePrivilege",null])))).unwrap();
        drain(&store);
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances WHERE lifecycle='active'"),2,"an invalid list entry leaves membership unknown");
        store.ingest(&account_snapshot(source,"guard-4","2026-10-13T04:00:00Z",items(json!([])))).unwrap();
        drain(&store);
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances WHERE lifecycle='not_observed'"),2,"a valid empty list covers both children");
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances WHERE lifecycle='removed'"),0,"absence is not deletion");
    }

    #[test]
    fn expanded_same_timestamp_snapshots_cannot_invent_an_order_or_revive_absence() {
        let mut store=Store::open(":memory:").unwrap();
        let source="pcscope.identity.assigned-rights";
        let items=|rights:Value|json!([{"id":"S-1-5-21-100","data":{"rights":rights}}]);
        let both=items(json!(["SeBackupPrivilege","SeRestorePrivilege"]));
        let one=items(json!(["SeRestorePrivilege"]));
        store.ingest(&account_snapshot(source,"tie-1","2026-10-13T01:00:00Z",both.clone())).unwrap();
        drain(&store);
        store.ingest(&account_snapshot(source,"tie-2","2026-10-13T01:00:00Z",one.clone())).unwrap();
        drain(&store);
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances WHERE lifecycle='active'"),2,"equal timestamps do not prove an earlier presence has ceased");
        let mut incomplete=account_snapshot(source,"partial","2026-10-13T02:00:00Z",one.clone());
        if let pcscope_contracts::Message::Snapshot {complete,..}=&mut incomplete.message {*complete=false;}
        assert!(store.ingest(&incomplete).is_err());
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances WHERE lifecycle='active'"),2);
        store.ingest(&account_snapshot(source,"tie-3","2026-10-13T02:00:00Z",one)).unwrap();
        drain(&store);
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances WHERE lifecycle='not_observed'"),1);
        store.ingest(&account_snapshot(source,"tie-4","2026-10-13T02:00:00Z",both.clone())).unwrap();
        drain(&store);
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances WHERE lifecycle='not_observed'"),1,"a different snapshot at the same time cannot supersede the accepted absence");
        store.ingest(&account_snapshot(source,"tie-5","2026-10-13T03:00:00Z",both)).unwrap();
        drain(&store);
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances WHERE lifecycle='active'"),2);
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances"),2,"a later sighting reuses the same non-deleted identities");
    }

    #[test]
    fn expanded_complete_parent_watermark_blocks_previously_unknown_old_children() {
        for (source, list_key, child_type, current_child, old_child, parent_count) in [
            ("pcscope.identity.assigned-rights","rights","account.right_assignment","SeBackupPrivilege","SeRestorePrivilege",0),
            ("pcscope.identity.groups","members","account.group_membership","S-1-5-21-100","S-1-5-21-200",1),
        ] {
            for empty in [false,true] {
                let mut store=Store::open(":memory:").unwrap();
                let item=|name:&str,children:Value| {
                    let mut payload=json!({"name":name});
                    payload[list_key]=children;
                    json!([{"id":"S-1-5-32-544","data":payload}])
                };
                let latest=if empty {json!([])} else {json!([current_child])};
                store.ingest(&account_snapshot(source,"new-list","2026-10-13T03:00:00Z",item("Current group",latest))).unwrap();
                drain(&store);
                assert_eq!(count(&store,"SELECT count(*) FROM settings WHERE key LIKE 'projection.expanded_complete_at.v1/%'"),1,"even an empty first list needs a persisted parent watermark");
                let expected=parent_count+if empty {0} else {1};
                assert_eq!(count(&store,"SELECT count(*) FROM object_instances"),expected);
                store.ingest(&account_snapshot(source,"late-old-list","2026-10-13T01:00:00Z",item("Old group",json!([old_child])))).unwrap();
                drain(&store);
                assert_eq!(count(&store,"SELECT count(*) FROM object_instances"),expected,"an unseen old child cannot become current after a newer complete list");
                assert_eq!(store.conn.query_row("SELECT count(*) FROM object_instances WHERE type_id=? AND native_key LIKE ?",params![child_type,format!("%{}",old_child.to_lowercase())],|r|r.get::<_,i64>(0)).unwrap(),0);
                assert_eq!(count(&store,"SELECT count(*) FROM source_bindings"),expected);
                assert_eq!(count(&store,"SELECT count(*) FROM snapshots"),2,"raw older evidence is retained");
                assert_eq!(pending(&store),0,"superseded evidence is processed, not retried forever");
                if parent_count==1 {
                    assert_eq!(fact(&store,1,"name").unwrap().0,json!("Current group").to_string());
                }
            }
        }
    }

    #[test]
    fn expanded_disabled_or_invalid_lists_cannot_advance_the_parent_watermark() {
        for (label,list,disabled) in [
            ("disabled",json!([]),true),
            ("null",Value::Null,false),
            ("mixed",json!(["SeRestorePrivilege",null]),false),
            ("mask",json!([pcscope_contracts::sensitive::MASK]),false),
            ("mixed-mask",json!(["SeRestorePrivilege",pcscope_contracts::sensitive::MASK]),false),
        ] {
            let mut store=Store::open(":memory:").unwrap();
            let source="pcscope.identity.assigned-rights";
            let items=|rights:Value|json!([{"id":"S-1-5-21-100","data":{"rights":rights}}]);
            store.ingest(&account_snapshot(source,"initial","2026-10-13T00:00:00Z",items(json!(["SeBackupPrivilege","SeRestorePrivilege"])))).unwrap();
            drain(&store);
            store.ingest(&account_snapshot(source,label,"2026-10-13T03:00:00Z",items(list))).unwrap();
            if disabled {
                let item=crate::item_controls::classify(source,"S-1-5-21-100","",&Value::Null);
                store.conn.execute("INSERT OR REPLACE INTO settings VALUES('item_settings',?)",[json!({"disabled":[item]}).to_string()]).unwrap();
            }
            drain(&store);
            assert_eq!(count(&store,"SELECT count(*) FROM object_instances WHERE lifecycle='not_observed'"),0,"{label} is not evidence of absence");
            let watermark:String=store.conn.query_row("SELECT value FROM settings WHERE key LIKE 'projection.expanded_complete_at.v1/%'",[],|r|r.get(0)).unwrap();
            assert_eq!(watermark,"2026-10-13T00:00:00Z","{label} cannot establish a newer complete-list boundary");
            store.conn.execute("DELETE FROM settings WHERE key='item_settings'",[]).unwrap();
            store.ingest(&account_snapshot(source,"late-valid","2026-10-13T01:00:00Z",items(json!(["SeBackupPrivilege","SeRestorePrivilege","SeDebugPrivilege"])))).unwrap();
            drain(&store);
            let id=instance_for_key(&store,"account.right_assignment","sedebugprivilege");
            assert_eq!(store.instance_summary(id).unwrap().unwrap()["lifecycle"],"active","{label} must not suppress a previously unknown child");
        }
    }

    #[test]
    fn expanded_parent_watermark_is_scoped_and_transactional() {
        let mut store=Store::open(":memory:").unwrap();
        let source="pcscope.identity.assigned-rights";
        store.ingest(&account_snapshot(source,"empty","2026-10-13T03:00:00Z",json!([{"id":"S-1-5-21-100","data":{"rights":[]}}]))).unwrap();
        {
            let tx=store.conn.unchecked_transaction().unwrap();
            project_pending(&tx,MACHINE,INSTALL,100).unwrap();
            assert_eq!(tx.query_row("SELECT count(*) FROM settings WHERE key LIKE 'projection.expanded_complete_at.v1/%'",[],|r|r.get::<_,i64>(0)).unwrap(),1);
            tx.rollback().unwrap();
        }
        assert_eq!(count(&store,"SELECT count(*) FROM settings WHERE key LIKE 'projection.expanded_complete_at.v1/%'"),0);
        assert_eq!(pending(&store),1,"watermark and queue completion roll back together");
        drain(&store);
        let mut input=store.evidence_inputs(0,10).unwrap().remove(0);
        input.observed_at="2026-10-13T01:00:00Z".into();
        input.payload=json!({"rights":["SeBackupPrivilege"]});
        let base=AdapterContext {
            catalog_version:pcscope_contracts::model::catalog_version().into(),rule_version:pcscope_contracts::model::catalog_fingerprint(),
            machine_id:MACHINE.into(),installation_id:INSTALL.into(),boot_id:None,collected_scope:business_scope(TASK_SCOPE),complete:true,
        };
        for dimension in ["parent","machine","installation","scope"] {
            let mut distinct_input=input.clone();
            let mut context=base.clone();
            match dimension {
                "parent"=>{distinct_input.item_id="S-1-5-21-200".into();distinct_input.native_record_id="S-1-5-21-200".into();},
                "machine"=>context.machine_id="other-machine".into(),
                "installation"=>context.installation_id="other-installation".into(),
                "scope"=>context.collected_scope="machine:another/install:another".into(),
                _=>unreachable!(),
            }
            let result=adapter_for(source).unwrap().adapt(&distinct_input,&context);
            assert!(apply(&store.conn,&distinct_input,&context,&result,&Default::default()).unwrap(),"different {dimension} must not inherit another parent's watermark");
        }
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances"),4);
        assert_eq!(count(&store,"SELECT count(*) FROM source_bindings"),4,"expanded bindings are also isolated by identity scope");
        assert_eq!(count(&store,"SELECT count(*) FROM settings WHERE key LIKE 'projection.expanded_complete_at.v1/%'"),5);
        let result=adapter_for(source).unwrap().adapt(&input,&base);
        assert!(!apply(&store.conn,&input,&base,&result,&Default::default()).unwrap(),"same parent still rejects the older list");
        let mut partition=base;
        partition.collected_scope=business_scope(&format!("{}/schema:99/context:other/shard:99",partition.collected_scope));
        let result=adapter_for(source).unwrap().adapt(&input,&partition);
        assert!(!apply(&store.conn,&input,&partition,&result,&Default::default()).unwrap(),"collection partition changes do not create a different object scope");
    }

    #[test]
    fn a_snapshot_projects_into_field_facts() {
        let mut store = Store::open(":memory:").unwrap();
        store.ingest(&tasks_snapshot("run-1", "2026-09-12T00:00:00Z", "示例任务", true)).unwrap();
        let report = drain(&store);
        assert_eq!(report.projected, 1);
        assert_eq!(pending(&store), 0);
        let instance = count(&store, "SELECT count(*) FROM object_instances");
        assert_eq!(instance, 1);
        let summary = store.instance_summary(1).unwrap().unwrap();
        assert_eq!(summary["type_id"], "startup.scheduled_task");
        let (value, status) = fact(&store, 1, "name").expect("name fact");
        assert_eq!(status, "known");
        assert_eq!(value, json!("示例任务").to_string());
        // The field cites the evidence it came from, so a fact is never unattributable.
        let refs = count(&store, "SELECT count(*) FROM evidence_refs WHERE subject_kind='observation'");
        assert!(refs > 0, "the observation must be recorded as applied to the instance");
    }

    #[test]
    fn a_late_older_observation_does_not_roll_the_current_value_back() {
        let mut store = Store::open(":memory:").unwrap();
        // The newer observation is stored and projected first; the older one arrives afterwards.
        store.ingest(&tasks_snapshot("run-2", "2026-09-12T02:00:00Z", "新名字", true)).unwrap();
        store.ingest(&tasks_snapshot("run-1", "2026-09-12T01:00:00Z", "旧名字", true)).unwrap();
        drain(&store);
        let (value, _) = fact(&store, 1, "name").expect("name fact");
        assert_eq!(value, json!("新名字").to_string(), "a late, older observation must not become the present");
    }

    #[test]
    fn an_unchanged_field_produces_no_change_action() {
        let mut store = Store::open(":memory:").unwrap();
        store.ingest(&tasks_snapshot("run-1", "2026-09-12T00:00:00Z", "示例任务", true)).unwrap();
        store.ingest(&tasks_snapshot("run-2", "2026-09-12T01:00:00Z", "示例任务", true)).unwrap();
        drain(&store);
        // Nothing changed, so nothing is reported as having changed.
        assert_eq!(count(&store, "SELECT count(*) FROM object_actions"), 0);
        assert_eq!(count(&store, "SELECT count(*) FROM fact_history"), 0);
    }

    #[test]
    fn a_changed_field_produces_one_interval_action() {
        let mut store = Store::open(":memory:").unwrap();
        store.ingest(&tasks_snapshot("run-1", "2026-09-12T00:00:00Z", "示例任务", true)).unwrap();
        store.ingest(&tasks_snapshot("run-2", "2026-09-12T01:00:00Z", "示例任务", false)).unwrap();
        drain(&store);
        let actions: Vec<(String, Option<String>, Option<String>, Option<String>, String)> = {
            let mut q = store
                .conn
                .prepare("SELECT action_id,interval_from,interval_until,before_value,precision FROM object_actions ORDER BY action_row")
                .unwrap();
            let rows = q.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))).unwrap();
            rows.collect::<rusqlite::Result<_>>().unwrap()
        };
        assert_eq!(actions.len(), 1, "one changed field is one action, not one per observation");
        let (action_id, from, until, before, precision) = &actions[0];
        assert_eq!(action_id, "observed.configuration_changed");
        assert_eq!(precision, "interval", "no source recorded the edit, so no exact moment is claimed");
        assert_eq!(from.as_deref(), Some("2026-09-12T00:00:00Z"));
        assert_eq!(until.as_deref(), Some("2026-09-12T01:00:00Z"));
        assert_eq!(before.as_deref(), Some("true"));
        assert_eq!(count(&store, "SELECT count(*) FROM fact_history"), 1);
    }

    #[test]
    fn corrected_com_mapping_establishes_a_baseline_before_recording_real_changes() {
        const SOURCE: &str = "pcscope.shell.registrations";
        const ROOT: &str = r"Software\Classes\CLSID\{0af96ede-aebf-41ed-a1c8-cf7a685505b6}";
        const LEGACY_RULE: &str = "154b198b4b59a0f3c2c26a39105bf11037f57e1baf44561bff1b11be6d8c181b";
        let snapshot = |run: &str, time: &str, server: Option<(&str, &str, &str)>| {
            let (path, data) = match server {
                Some((file, version, hash)) => {
                    let path = format!(r"{ROOT}\InprocServer32");
                    let data = json!({"registry_path":path,"values":{"":{"kind":"ExpandString","value":file}},
                        "implementation_file":{"status":"observed","path":file,"file_version":version,"sha256":hash}});
                    (path, data)
                },
                None => (ROOT.into(), json!({"registry_path":ROOT,"values":{"":{"kind":"String","value":"Library Folder Context Menu"}}})),
            };
            envelope(json!({"protocol_version":1,"request_id":run,"type":"snapshot","source":SOURCE,
                "scope":"machine:m/LocalMachine/Registry64","run_id":run,"observed_at":time,"complete":true,
                "items":[{"id":path.to_lowercase(),"data":data}]}))
        };
        let mut store = Store::open(":memory:").unwrap();
        store.ingest(&snapshot("root", "2026-10-13T01:00:00Z", None)).unwrap();
        assert_eq!(drain(&store).rejected, 0);
        let instance = instance_for_key(&store, "shell.com_registration", "{0af96ede-aebf-41ed-a1c8-cf7a685505b6}");
        // Recreate the persisted facts of the old mapping; the actual root material remains
        // available as evidence, including the description previously used as server_path.
        assert!(crate::query::is_legacy_com_rule(LEGACY_RULE));
        for (field, value) in [("server_path", "Library Folder Context Menu"), ("file_version", "old-version"), ("sha256", "old-hash")] {
            store.conn.execute(
                "INSERT INTO object_facts(instance_id,field_id,field_kind,value,value_type,value_status,observed_at,derivation,rule_version)
                 VALUES(?,?,'configuration',?,'string','known','2026-10-13T01:00:00Z','source_provided',?)",
                params![instance, field, json!(value).to_string(), LEGACY_RULE],
            ).unwrap();
        }
        let first = (r"C:\Windows\system32\shell32.dll", "new-version", "new-hash");
        store.ingest(&snapshot("corrected", "2026-10-13T02:00:00Z", Some(first))).unwrap();
        assert_eq!(drain(&store).rejected, 0);
        assert_eq!(count(&store, "SELECT count(*) FROM object_actions"), 0, "mapping correction is not a system change");
        assert_eq!(count(&store, "SELECT count(*) FROM fact_history"), 0, "the legacy value cannot supply a trusted before-value");
        for (field, value) in [("server_path", first.0), ("file_version", first.1), ("sha256", first.2)] {
            assert_eq!(fact(&store, instance, field), Some((json!(value).to_string(), "known".into())));
            let rule: String = store.conn.query_row("SELECT rule_version FROM object_facts WHERE instance_id=? AND field_id=?", params![instance, field], |row| row.get(0)).unwrap();
            assert!(!crate::query::is_legacy_com_rule(&rule), "{field} must now use the corrected baseline");
        }
        assert_eq!(count(&store, "SELECT count(*) FROM evidence_refs WHERE subject_kind='observation'"), 2);
        assert!(store.evidence_inputs(0, 10).unwrap().iter().any(|input|
            input.payload["registry_path"] == ROOT && input.payload["values"][""]["value"] == "Library Folder Context Menu"));

        let next = (r"C:\Windows\system32\updated.dll", "next-version", "next-hash");
        store.ingest(&snapshot("real-change", "2026-10-13T03:00:00Z", Some(next))).unwrap();
        assert_eq!(drain(&store).rejected, 0);
        assert_eq!(count(&store, "SELECT count(*) FROM object_actions"), 3, "subsequent real implementation changes retain their actions");
        assert_eq!(count(&store, "SELECT count(*) FROM fact_history"), 3);
        let before: String = store.conn.query_row("SELECT before_value FROM object_actions WHERE field_id='server_path'", [], |row| row.get(0)).unwrap();
        assert_eq!(before, json!(first.0).to_string(), "the new baseline, not the description, is compared");

        // A legacy rule marker must not bypass effective-time selection or display protection.
        store.conn.execute("UPDATE object_facts SET rule_version=? WHERE instance_id=? AND field_id='server_path'", params![LEGACY_RULE, instance]).unwrap();
        store.ingest(&snapshot("late", "2026-10-13T02:30:00Z", Some((first.0, next.1, next.2)))).unwrap();
        assert_eq!(drain(&store).rejected, 0);
        assert_eq!(fact(&store, instance, "server_path").unwrap().0, json!(next.0).to_string());
        assert_eq!(count(&store, "SELECT count(*) FROM object_actions"), 3);
        store.conn.execute("UPDATE object_facts SET value=?,value_status='masked' WHERE instance_id=? AND field_id='server_path'", params![json!("******").to_string(), instance]).unwrap();
        store.ingest(&snapshot("protected", "2026-10-13T04:00:00Z", Some(next))).unwrap();
        assert_eq!(drain(&store).rejected, 0);
        assert_eq!(fact(&store, instance, "server_path"), Some((json!("******").to_string(), "masked".into())));
        assert_eq!(count(&store, "SELECT count(*) FROM object_actions"), 3);
    }

    #[test]
    fn com_keys_in_one_snapshot_merge_all_roles_and_replay_each_item_once() {
        const ROOT: &str = r"Software\Classes\CLSID\{0af96ede-aebf-41ed-a1c8-cf7a685505b6}";
        const FILE: &str = r"C:\Windows\system32\shell32.dll";
        const COMMAND: &str = r#""C:\Example\server.exe" /Embedding"#;
        let snapshot = |run: &str, time: &str, order: &[usize; 3], file: &str| {
            let inproc = format!(r"{ROOT}\InprocServer32");
            let local = format!(r"{ROOT}\LocalServer32");
            let items = [
                json!({"id":ROOT.to_lowercase(),"data":{"registry_path":ROOT,"values":{"":{"kind":"String","value":"Library Folder Context Menu"}}}}),
                json!({"id":inproc.to_lowercase(),"data":{"registry_path":inproc,"values":{"":{"kind":"ExpandString","value":file}},
                    "implementation_file":{"status":"observed","path":file,"file_version":"1.0","sha256":"unchanged-hash"}}}),
                json!({"id":local.to_lowercase(),"data":{"registry_path":local,"values":{"":{"kind":"String","value":COMMAND}}}}),
            ];
            envelope(json!({"protocol_version":1,"request_id":run,"type":"snapshot","source":"pcscope.shell.registrations",
                "scope":"machine:m/LocalMachine/Registry64/shard:0","run_id":run,"observed_at":time,"complete":true,
                "items":order.iter().map(|index|items[*index].clone()).collect::<Vec<_>>()}))
        };
        let mut store = Store::open(":memory:").unwrap();
        let orders = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
        for (index, order) in orders.iter().enumerate() {
            store.ingest(&snapshot(&format!("same-snapshot-{index}"), &format!("2026-10-13T{:02}:00:00Z", index + 1), order, FILE)).unwrap();
            let report = drain(&store);
            assert_eq!(report.rejected, 0);
            assert_eq!(report.projected, 3, "each registry key role must be applied, in order {order:?}");
            assert_eq!(count(&store, "SELECT count(*) FROM object_instances"), 1);
            let instance = instance_for_key(&store, "shell.com_registration", "{0af96ede-aebf-41ed-a1c8-cf7a685505b6}");
            for (field, value) in [("name", "Library Folder Context Menu"), ("server_path", FILE), ("local_server_command", COMMAND), ("file_version", "1.0"), ("sha256", "unchanged-hash")] {
                assert_eq!(fact(&store, instance, field), Some((json!(value).to_string(), "known".into())), "{field}, order {order:?}");
            }
        }
        assert_eq!(count(&store, "SELECT count(*) FROM object_actions"), 0, "unchanged roles never alternate values");
        assert_eq!(count(&store, "SELECT count(*) FROM fact_history"), 0);
        assert_eq!(count(&store, "SELECT count(*) FROM evidence_refs WHERE subject_kind='observation'"), 6, "ordinary snapshot evidence remains available once per instance");
        assert_eq!(count(&store, "SELECT count(*) FROM evidence_refs WHERE subject_kind='observation_item'"), 18);
        let updated = r"C:\Windows\system32\updated.dll";
        store.ingest(&snapshot("real-change", "2026-10-13T07:00:00Z", &[0, 2, 1], updated)).unwrap();
        assert_eq!(drain(&store).projected, 3);
        assert_eq!(count(&store, "SELECT count(*) FROM object_actions"), 1);
        assert_eq!(count(&store, "SELECT count(*) FROM fact_history"), 1);
        let refs = count(&store, "SELECT count(*) FROM evidence_refs");
        store.conn.execute("UPDATE projection_queue SET state='pending'", []).unwrap();
        let replay = drain(&store);
        assert_eq!(replay.rejected, 0);
        assert_eq!(replay.projected, 0);
        assert_eq!(replay.duplicate, 21);
        assert_eq!(count(&store, "SELECT count(*) FROM object_actions"), 1);
        assert_eq!(count(&store, "SELECT count(*) FROM fact_history"), 1);
        assert_eq!(count(&store, "SELECT count(*) FROM evidence_refs"), refs);
        let instance = instance_for_key(&store, "shell.com_registration", "{0af96ede-aebf-41ed-a1c8-cf7a685505b6}");
        assert_eq!(fact(&store, instance, "server_path").unwrap().0, json!(updated).to_string(), "replaying older snapshots cannot restore the old file");
    }

    #[test]
    fn two_sources_that_cut_the_same_task_up_differently_land_on_one_instance() {
        let mut store = Store::open(":memory:").unwrap();
        // The base inventory and the detailed configuration of one task. Everything about the task
        // is the same; only the collection context and shard differ, and those describe how the
        // collection was cut up, not what the object belongs to.
        store.ingest(&tasks_snapshot("run-1", "2026-09-12T00:00:00Z", "示例任务", true)).unwrap();
        store
            .ingest(&envelope(json!({
                "protocol_version": 1, "request_id": "run-2", "type": "snapshot",
                "source": "pcscope.startup.task-details",
                "scope": "machine:m/install:i/schema:1/context:task-detail",
                "run_id": "run-2", "observed_at": "2026-09-12T00:00:00Z", "complete": true,
                "items": [{"id": TASK_PATH, "data": {"path": TASK_PATH, "name": "示例任务"}}]
            })))
            .unwrap();
        let report = drain(&store);
        assert_eq!(report.projected, 2);
        assert_eq!(count(&store, "SELECT count(*) FROM object_instances"), 1, "one task, two sources, one instance");
        // Both sources are bound to it, and the field cites both of their evidence.
        assert_eq!(count(&store, "SELECT count(*) FROM source_bindings WHERE valid_to IS NULL"), 2);
        let cited: i64 = store
            .conn
            .query_row("SELECT count(DISTINCT evidence_id) FROM evidence_refs WHERE subject_kind='observation'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(cited, 2, "the field's observation cites each source's evidence, not just the first");
    }

    #[test]
    fn values_that_disagree_at_one_time_are_kept_as_a_conflict() {
        let mut store = Store::open(":memory:").unwrap();
        // Two complete observations of the same scope at the same effective time, disagreeing.
        store.ingest(&tasks_snapshot("run-1", "2026-09-12T00:00:00Z", "甲", true)).unwrap();
        store.ingest(&tasks_snapshot("run-2", "2026-09-12T00:00:00Z", "乙", true)).unwrap();
        drain(&store);
        let (status, conflicts): (String, Option<String>) = store
            .conn
            .query_row("SELECT value_status,conflict_sources FROM object_facts WHERE field_id='name'", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        // Neither value silently wins: the disagreement is the result, and both candidates survive.
        assert_eq!(status, "conflicting");
        let conflicts = conflicts.expect("both candidates are kept");
        assert!(conflicts.contains("甲") && conflicts.contains("乙"), "{conflicts}");
    }

    #[test]
    fn re_processing_the_same_evidence_writes_nothing_new() {
        let mut store = Store::open(":memory:").unwrap();
        store.ingest(&tasks_snapshot("run-1", "2026-09-12T00:00:00Z", "示例任务", true)).unwrap();
        store.ingest(&tasks_snapshot("run-2", "2026-09-12T01:00:00Z", "示例任务", false)).unwrap();
        drain(&store);
        let before = (
            count(&store, "SELECT count(*) FROM object_facts"),
            count(&store, "SELECT count(*) FROM object_actions"),
            count(&store, "SELECT count(*) FROM fact_history"),
        );
        // The queue is put back the way a crash before the commit would have left it, and drained
        // again. The same evidence has to produce the same state, not a second copy of it.
        store.conn.execute("UPDATE projection_queue SET state='pending'", []).unwrap();
        let report = drain(&store);
        assert_eq!(report.duplicate + report.projected, 2);
        let after = (
            count(&store, "SELECT count(*) FROM object_facts"),
            count(&store, "SELECT count(*) FROM object_actions"),
            count(&store, "SELECT count(*) FROM fact_history"),
        );
        assert_eq!(before, after, "re-processing must not duplicate facts, actions or history");
    }

    #[test]
    fn a_source_without_an_adapter_is_recorded_rather_than_dropped() {
        let mut store = Store::open(":memory:").unwrap();
        store
            .ingest(&envelope(json!({
                "protocol_version": 1, "request_id": "g-1", "type": "gap", "source": TASKS, "reason": "denied", "observed_at": "2026-09-12T00:00:00Z"
            })))
            .unwrap();
        store
            .ingest(&envelope(json!({
                "protocol_version": 1, "request_id": "s-1", "type": "snapshot", "source": "pcscope.unconverted.thing",
                "scope": "machine:m", "run_id": "s-1", "observed_at": "2026-09-12T00:00:00Z", "complete": true,
                "items": [{"id": "x", "data": {"any": "thing"}}]
            })))
            .unwrap();
        let report = drain(&store);
        assert_eq!(report.unadapted, 1);
        let reason: String = store
            .conn
            .query_row("SELECT last_error FROM projection_queue WHERE state='failed'", [], |r| r.get(0))
            .unwrap();
        assert!(reason.contains("no adapter for source pcscope.unconverted.thing"), "{reason}");
        // The material itself is still stored and still readable: an unadapted source is a gap in
        // the model, not a reason to lose what was collected.
        let inputs = store.evidence_inputs(0, 10).unwrap();
        assert!(inputs.iter().any(|input| input.source == "pcscope.unconverted.thing"));
    }

    #[test]
    fn a_start_and_an_end_are_two_actions() {
        let mut store = Store::open(":memory:").unwrap();
        store.ingest(&task_run_events()).unwrap();
        drain(&store);
        let ids: Vec<String> = {
            let mut q = store.conn.prepare("SELECT action_id FROM object_actions ORDER BY action_row").unwrap();
            let rows = q.query_map([], |r| r.get(0)).unwrap();
            rows.collect::<rusqlite::Result<_>>().unwrap()
        };
        assert_eq!(ids, vec!["startup.task_started".to_owned(), "startup.task_ended".to_owned()],
            "a start and an end are two stages and must never be deduplicated into one");
    }
}
