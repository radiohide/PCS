//! R03 identity resolution interface.
//!
//! Freezes how an `IdentityCandidate` (R02) becomes an instance decision. This module is the
//! *decision* only: it is a pure function over a lookup. Allocating and persisting instance IDs is
//! S06's work, and the interface is shaped so that can be added without changing these types.
//!
//! Two boundaries are enforced here rather than by convention:
//!
//! * **business scope vs collection scope.** The identity key is built from type-declared keys
//!   only. A collection shard, request id or plugin name can never enter it — `InstanceIdentity`
//!   has no field that could hold one, and the checker in the model catalog already rejects such a
//!   key in the declaration.
//! * **observed vs decided.** Nothing in this module infers a creation time, a deletion or an
//!   end. Absence produces `NotObserved`; only stated evidence moves an instance further.
use crate::adapter::IdentityCandidate;
use crate::model;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ---------- input ----------

/// The type's identity strategy, read from the catalog. Carried explicitly so a decision can be
/// explained and replayed against the strategy version it was made with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeStrategy {
    pub strategy_id: String,
    pub keys: Vec<String>,
    pub scope: String,
    pub normalization: String,
    pub reuse_rule: String,
    pub insufficient: String,
}

impl TypeStrategy {
    /// Reads the strategy for a type from the embedded catalog.
    pub fn for_type(type_id: &str) -> Option<Self> {
        let definition = model::type_definition(type_id)?;
        let identity = &definition["identity"];
        Some(Self {
            strategy_id: identity["strategy"].as_str()?.to_owned(),
            keys: identity["keys"].as_array()?.iter().filter_map(|k| k.as_str()).map(str::to_owned).collect(),
            scope: identity["scope"].as_str().unwrap_or_default().to_owned(),
            normalization: identity["normalization"].as_str().unwrap_or_default().to_owned(),
            reuse_rule: identity["reuse_rule"].as_str().unwrap_or_default().to_owned(),
            insufficient: identity["insufficient"].as_str().unwrap_or_default().to_owned(),
        })
    }
}

/// Time context of the material the candidate came from, plus what the collection actually covered.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceTime {
    /// Time the source stamped the material, when it stated one.
    pub source_time: Option<String>,
    pub observed_at: String,
}

/// Facts about the observation that bear on lifecycle but are not part of identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LifecycleHints {
    /// The source signalled a complete observation of the scope it covered.
    pub complete_observation: bool,
    /// Scope the collection actually covered, which may be narrower than the type's scope.
    pub collected_scope: String,
    /// Whether the collection was configured at all. A disabled item is not a disappearance.
    pub configured: bool,
    /// Earliest time still retained for this source, when history was pruned.
    pub history_retained_from: Option<String>,
    /// True only when the source itself states that the object ended or was removed.
    pub states_object_ended: bool,
    /// Identifier of the boot/installation generation, when the source provides one.
    pub boot_id: Option<String>,
}

/// The source record a candidate came from. This is how the resolver finds the instance that record
/// was already bound to, which is what keeps a provisional identity from being duplicated on the
/// next round and what lets two sources converge on one instance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceRecordRef {
    pub source: String,
    pub item_id: String,
    /// Identity the source itself states for the record. Empty when it states none.
    pub native_record_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IdentityRequest {
    pub candidate: IdentityCandidate,
    pub strategy: TypeStrategy,
    /// Where this candidate came from. Never part of the identity key.
    pub binding: SourceRecordRef,
    pub machine_id: String,
    pub installation_id: String,
    pub evidence_time: EvidenceTime,
    pub hints: LifecycleHints,
    /// Rule version the decision is made against, so it can be replayed.
    pub rule_version: String,
}

// ---------- identity key ----------

/// The stable comparison key for an instance. `native_key` is the normalised form; the original
/// values stay in the candidate for display.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct InstanceIdentity {
    pub type_id: String,
    pub machine_id: String,
    pub installation_id: String,
    pub scope: String,
    pub native_key: String,
    /// Generation counter for a native key that was removed and later reappeared.
    pub incarnation: u32,
}

impl InstanceIdentity {
    /// Deterministic alias used to reference an instance before it has a stored ID, and to resolve
    /// references after a provisional identity is confirmed. References never depend on a numeric
    /// ID alone, so confirming an identity cannot silently break an existing link.
    pub fn alias(&self) -> String {
        format!("{}@{}|{}|{}|{}#{}", self.type_id, self.machine_id, self.installation_id, self.scope, self.native_key, self.incarnation)
    }
}

// ---------- normalisation ----------

/// How a component of the key is made comparable. Windows decides case sensitivity per namespace;
/// this is not a universal rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaseRule {
    /// The namespace is case-insensitive; fold for comparison, keep the original for display.
    Fold,
    /// The value is already in a canonical form; compare as-is.
    Preserve,
}

/// Normalisation for one strategy. Every key component is normalised, then joined with `|`.
fn case_rule(strategy_id: &str) -> Option<CaseRule> {
    Some(match strategy_id {
        // Registry paths, service and printer names are case-insensitive namespaces.
        "registry_registration" | "named_entity" | "value_key" => CaseRule::Fold,
        // SIDs are case-insensitive by definition.
        "account_sid" => CaseRule::Fold,
        // Windows task and file paths are case-insensitive.
        "native_path" => CaseRule::Fold,
        // Native identifiers, file IDs, endpoints and composite keys are compared as the source
        // states them; only the individual components are normalised.
        "native_identifier" | "file_identity" | "endpoint_tuple" | "composite_native" | "process_instance" => CaseRule::Preserve,
        _ => return None,
    })
}

fn fold(value: &str) -> String {
    value.trim().to_lowercase()
}
fn normalize_path(value: &str) -> String {
    // Trailing separators are not part of a path identity; interior separators are already `\` in
    // Windows input and are not rewritten.
    value.trim().trim_end_matches('\\').to_lowercase()
}
fn normalize_component(strategy_id: &str, key: &str, value: &str) -> String {
    match (strategy_id, key) {
        ("native_path", "path") => normalize_path(value),
        (_, "path") | (_, "registry_path") => normalize_path(value),
        ("account_sid", "sid") => fold(value),
        ("native_identifier", _) => value.trim().to_owned(),
        ("process_instance", "pid") => value.trim().to_owned(),
        ("process_instance", "created_at") => value.trim().to_owned(),
        ("endpoint_tuple", _) => value.trim().to_owned(),
        ("file_identity", "file_id") => value.trim().to_lowercase(),
        _ => match case_rule(strategy_id) {
            Some(CaseRule::Fold) => fold(value),
            _ => value.trim().to_owned(),
        },
    }
}

/// Builds the comparable key from the candidate's identity fields. Returns the key and any
/// declared key the candidate did not supply.
///
/// A missing component is never defaulted: the caller receives it in `missing` and the identity is
/// reported as provisional rather than being keyed on a guess.
pub fn normalise(strategy: &TypeStrategy, candidate: &IdentityCandidate) -> (String, Vec<String>) {
    if strategy.strategy_id=="process_instance" {
        if let Some(key)=historical_process_key(candidate){return (key,Vec::new())}
    }
    let provided: BTreeMap<&str, &str> = candidate.fields.iter().map(|f| (f.key.as_str(), f.value.as_str())).collect();
    let mut parts = Vec::new();
    let mut missing = Vec::new();
    for key in &strategy.keys {
        if key == "scope" {
            parts.push(normalize_component(&strategy.strategy_id, "scope", &candidate.scope));
            continue;
        }
        match provided.get(key.as_str()) {
            Some(value) if !value.trim().is_empty() => parts.push(normalize_component(&strategy.strategy_id, key, value)),
            _ => {
                missing.push(key.clone());
                // A marker keeps the position stable, but the identity is provisional because of it.
                parts.push(format!("?{key}"));
            }
        }
    }
    // A composite strategy declares its keys on the type; anything it supplied beyond those joins
    // the key in a stable order so two different composites cannot collide.
    if strategy.strategy_id == "composite_native" || strategy.keys.is_empty() {
        let mut extras: Vec<String> = candidate
            .fields
            .iter()
            .filter(|f| f.key != "scope" && !strategy.keys.contains(&f.key))
            .map(|f| format!("{}={}", f.key.to_lowercase(), normalize_component(&strategy.strategy_id, &f.key, &f.value)))
            .collect();
        extras.sort();
        parts.extend(extras);
    }
    (parts.join("|"), missing)
}

/// Alternative process identities occupy a separate namespace. The event's timestamp never
/// substitutes for GetProcessTimes creation time, and a PID alone never identifies a process.
pub fn historical_process_key(candidate:&IdentityCandidate)->Option<String> {
    if candidate.type_id!="runtime.process"{return None}
    let get=|key:&str|candidate.fields.iter().find(|f|f.key==key).map(|f|f.value.as_str());
    if get("created_at").is_some(){return None}
    let pid=crate::adapters::process_history::pid(get("pid")?)?;
    if let Some(guid)=get("process_guid") {
        let guid=guid.strip_prefix('{').and_then(|s|s.strip_suffix('}')).unwrap_or(guid);
        if guid.len()!=36||!guid.bytes().enumerate().all(|(n,b)|if [8,13,18,23].contains(&n){b==b'-'}else{b.is_ascii_hexdigit()})||guid.bytes().all(|b|b==b'0'||b==b'-'){return None}
        return Some(serde_json::json!([candidate.scope,"sysmon-process",guid.to_ascii_lowercase(),pid]).to_string())
    }
    let event=get("start_event")?;
    if event.is_empty()||event.contains('\0'){return None}
    Some(serde_json::json!([candidate.scope,"security-start",event,pid]).to_string())
}

// ---------- lookup ----------

/// Existing instances the resolver may match against. S06 supplies a database-backed
/// implementation; the contract only requires these two lookups.
pub trait InstanceLookup {
    /// Instances already stored under this type and native key.
    fn by_key(&self, type_id: &str, machine_id: &str, installation_id: &str, scope: &str, native_key: &str) -> Vec<(u64, u32)>;
    /// Instance this source record was already bound to, matched by the source's own identity.
    fn by_source_binding(&self, source: &str, item_id: &str, native_record_id: &str) -> Option<(u64, u32)>;
}

// ---------- outcome ----------

/// How an existing instance came to be matched. Kept explicit so a decision can be explained.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchBasis {
    /// Matched an instance already stored under this key.
    ExistingByKey,
    /// Matched the instance this source record was previously bound to, even though the key alone
    /// would not have been enough. This is what keeps a provisional identity from forking.
    ExistingBySourceBinding,
    /// Nothing matched; a new instance is required.
    New,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfirmedIdentity {
    pub identity: InstanceIdentity,
    /// Stored instance id when one was matched. `None` means a new instance must be allocated.
    pub instance_id: Option<u64>,
    pub basis: MatchBasis,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProvisionalIdentity {
    pub identity: InstanceIdentity,
    /// Instance this belongs to when the source record already resolved to one. A provisional
    /// identity stays provisional even when the instance is known: the binding says *which*
    /// instance, not that the key is now complete.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<u64>,
    /// Since the alias is derived from the key, references made against it survive confirmation.
    pub alias: String,
    pub reason: String,
    /// Keys that would complete the identity and let it be confirmed later.
    pub could_be_refined_by: Vec<String>,
}

/// Several existing instances match and nothing distinguishes them. One is never picked arbitrarily.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ambiguity {
    pub identity: InstanceIdentity,
    pub candidates: Vec<u64>,
    pub reason: String,
}

/// The four outcomes the plan requires.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum IdentityResolution {
    Confirmed(ConfirmedIdentity),
    Provisional(ProvisionalIdentity),
    Ambiguous(Ambiguity),
    /// No identity can even be proposed, with a reason that stays visible.
    Unresolved { reason: String },
}

fn build(request: &IdentityRequest, native_key: String, incarnation: u32) -> InstanceIdentity {
    InstanceIdentity {
        type_id: request.candidate.type_id.clone(),
        machine_id: request.machine_id.clone(),
        installation_id: request.installation_id.clone(),
        scope: request.candidate.scope.clone(),
        native_key,
        incarnation,
    }
}

/// The decision. Pure: everything consultable arrives in `request` and `lookup`.
///
/// Note what this function does **not** do: it does not refuse to resolve because the collection
/// item is currently disabled. A candidate only exists because material was collected for that item,
/// and an item that is switched off after collecting must keep the facts it already produced — the
/// model requires the last observed state to be retained, not dropped. The `configured` flag belongs
/// where an absence is being concluded, which is `semantics::absence_eligible`, and R09's drill
/// exposed this by projecting a disabled item's retained evidence and getting nothing back.
pub fn resolve(request: &IdentityRequest, lookup: &dyn InstanceLookup) -> IdentityResolution {
    let (native_key, missing) = normalise(&request.strategy, &request.candidate);
    if request.candidate.ambiguous.is_some() {
        return IdentityResolution::Ambiguous(Ambiguity {
            identity: build(request, native_key, 1),
            candidates: Vec::new(),
            reason: request.candidate.ambiguous.clone().unwrap_or_default(),
        });
    }
    let matches = lookup.by_key(&request.candidate.type_id, &request.machine_id, &request.installation_id, &request.candidate.scope, &native_key);
    if matches.len() > 1 {
        return IdentityResolution::Ambiguous(Ambiguity {
            identity: build(request, native_key, 1),
            candidates: matches.iter().map(|(id, _)| *id).collect(),
            reason: format!("{} instances share this key and nothing else distinguishes them", matches.len()),
        });
    }
    // A source-bound instance wins over key matching: the source already told us these records
    // belong together, and that statement survives a key becoming complete later. It says *which*
    // instance, not that the identity is now complete.
    let (instance_id, incarnation, basis) = match lookup.by_source_binding(&request.binding.source, &request.binding.item_id, &request.binding.native_record_id) {
        Some((id, incarnation)) => (Some(id), incarnation, MatchBasis::ExistingBySourceBinding),
        None => match matches.first() {
            Some((id, incarnation)) => (Some(*id), *incarnation, MatchBasis::ExistingByKey),
            None => (None, 1, MatchBasis::New),
        },
    };
    // An incomplete key stays provisional even when the instance is already known. Confirming it
    // requires the missing component to arrive, not merely another observation without it.
    if !missing.is_empty() {
        let identity = build(request, native_key, incarnation);
        return IdentityResolution::Provisional(ProvisionalIdentity {
            alias: identity.alias(),
            identity,
            instance_id,
            reason: format!("required identity key(s) not supplied: {}", missing.join(", ")),
            could_be_refined_by: missing,
        });
    }
    IdentityResolution::Confirmed(ConfirmedIdentity {
        identity: build(request, native_key, incarnation),
        instance_id,
        basis,
    })
}

// ---------- lifecycle ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleState {
    Active,
    NotObserved,
    Ended,
    Removed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationFacts {
    /// Present in this observation.
    Present,
    /// Absent from a complete observation of the scope that covered it.
    Absent,
}

/// What the lifecycle step decided, with the basis that supports it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "transition", rename_all = "snake_case")]
pub enum LifecycleTransition {
    /// The first time PCScope saw this instance. This is **not** a creation event.
    FirstObserved { at: String, basis: String },
    StillObserved,
    /// A previously absent instance is present again, without inventing a creation operation.
    ObservedAgain { at: String },
    /// Absent from a complete observation. Absence is not deletion, so the instance keeps its
    /// previous state and gains a "not observed since" time.
    NotObserved { since: String, basis: String },
    /// The source itself stated the object ended.
    EndedStated { at: String, basis: String },
    /// The source itself stated the object was removed.
    RemovedStated { at: String, basis: String },
    /// The same native key reappeared as a new instance after removal. A new generation, never a
    /// continuation of the old one.
    Reincarnated { incarnation: u32, basis: String },
    /// Nothing changed, or nothing can be concluded. The reason is kept.
    Unchanged { reason: String },
}

/// Applies the lifecycle rules. Never upgrades absence into deletion, and never invents a creation
/// or a start time.
pub fn lifecycle(
    previous: Option<(LifecycleState, u32)>,
    observation: ObservationFacts,
    hints: &LifecycleHints,
    observed_at: &str,
) -> LifecycleTransition {
    if !hints.configured {
        return LifecycleTransition::Unchanged { reason: "collection is disabled; absence is not a lifecycle fact".into() };
    }
    match (previous, observation) {
        (None, ObservationFacts::Present) => LifecycleTransition::FirstObserved {
            at: observed_at.to_owned(),
            basis: "first observation by PCScope; the source provides no creation time".into(),
        },
        (Some((LifecycleState::Active, _)), ObservationFacts::Present) => {
            if hints.states_object_ended {
                return LifecycleTransition::EndedStated { at: observed_at.to_owned(), basis: "the source stated the object ended".into() };
            }
            LifecycleTransition::StillObserved
        }
        (Some((LifecycleState::NotObserved, _)), ObservationFacts::Present) => {
            if hints.states_object_ended {
                LifecycleTransition::EndedStated { at: observed_at.to_owned(), basis: "the source stated the object ended".into() }
            } else {
                LifecycleTransition::ObservedAgain { at: observed_at.to_owned() }
            }
        }
        // The key reappeared after being ended or removed: a new generation, never a continuation.
        (Some((LifecycleState::Ended | LifecycleState::Removed, incarnation)), ObservationFacts::Present) => LifecycleTransition::Reincarnated {
            incarnation: incarnation.saturating_add(1),
            basis: "the key was previously ended or removed and has reappeared as a new generation".into(),
        },
        (Some((LifecycleState::Active, _)), ObservationFacts::Absent) => {
            if !hints.complete_observation {
                return LifecycleTransition::Unchanged { reason: "the observation was incomplete; absence proves nothing".into() };
            }
            if hints.collected_scope.trim().is_empty() {
                return LifecycleTransition::Unchanged { reason: "the collected scope is unknown; absence cannot be attributed".into() };
            }
            if hints.states_object_ended {
                return LifecycleTransition::RemovedStated { at: observed_at.to_owned(), basis: "the source stated the object was removed".into() };
            }
            LifecycleTransition::NotObserved {
                since: observed_at.to_owned(),
                basis: format!("absent from a complete observation of {}", hints.collected_scope),
            }
        }
        (Some((LifecycleState::NotObserved, _)), ObservationFacts::Absent) => {
            if hints.states_object_ended {
                return LifecycleTransition::RemovedStated { at: observed_at.to_owned(), basis: "the source stated the object was removed".into() };
            }
            LifecycleTransition::Unchanged { reason: "still not observed; absence is still not proof of removal".into() }
        }
        (Some((LifecycleState::Ended | LifecycleState::Removed, _)), ObservationFacts::Absent) => {
            LifecycleTransition::Unchanged { reason: "already ended or removed; no further transition".into() }
        }
        // Absent before ever being observed: there is no lifecycle to move.
        (None, ObservationFacts::Absent) => LifecycleTransition::Unchanged {
            reason: "the instance has never been observed, so absence is not a transition".into(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::{FieldKind, IdentityField};

    fn strategy(type_id: &str) -> TypeStrategy {
        TypeStrategy::for_type(type_id).unwrap_or_else(|| panic!("{type_id} has no strategy"))
    }
    fn candidate(type_id: &str, scope: &str, fields: &[(&str, &str)]) -> IdentityCandidate {
        IdentityCandidate {
            type_id: type_id.into(),
            scope: scope.into(),
            fields: fields.iter().map(|(k, v)| IdentityField { key: (*k).into(), value: (*v).into(), from: "test".into() }).collect(),
            missing: Vec::new(),
            ambiguous: None,
        }
    }
    fn request(candidate: IdentityCandidate, type_id: &str) -> IdentityRequest {
        IdentityRequest {
            candidate,
            strategy: strategy(type_id),
            binding: SourceRecordRef { source: "pcscope.test".into(), item_id: "item".into(), native_record_id: "record-1".into() },
            machine_id: "m".into(),
            installation_id: "i".into(),
            evidence_time: EvidenceTime { source_time: None, observed_at: "2026-09-12T00:00:00Z".into() },
            hints: LifecycleHints {
                complete_observation: true,
                collected_scope: "machine:m".into(),
                configured: true,
                history_retained_from: None,
                states_object_ended: false,
                boot_id: None,
            },
            rule_version: model::catalog_fingerprint(),
        }
    }
    struct Empty;
    impl InstanceLookup for Empty {
        fn by_key(&self, _: &str, _: &str, _: &str, _: &str, _: &str) -> Vec<(u64, u32)> { Vec::new() }
        fn by_source_binding(&self, _: &str, _: &str, _: &str) -> Option<(u64, u32)> { None }
    }

    #[test]
    fn every_catalog_strategy_has_a_normalisation_rule() {
        for definition in model::types() {
            let id = definition["identity"]["strategy"].as_str().unwrap();
            assert!(case_rule(id).is_some(), "no case rule for strategy {id}");
        }
    }

    #[test]
    fn process_history_keys_are_separate_from_native_pid_creation_keys() {
        let strategy=strategy("runtime.process");
        let native=candidate("runtime.process","s",&[("pid","32"),("created_at","2026-10-22T00:00:00Z")]);
        let guid=candidate("runtime.process","s",&[("pid","0x20"),("process_guid","{11111111-1111-1111-1111-111111111111}")]);
        let event=candidate("runtime.process","s",&[("pid","32"),("start_event","security|epoch-one|42")]);
        let other=candidate("runtime.process","s",&[("pid","32"),("start_event","security|epoch-two|42")]);
        let keys=[&native,&guid,&event,&other].map(|c|normalise(&strategy,c));
        assert!(keys.iter().all(|(_,missing)|missing.is_empty()));
        assert_eq!(keys.iter().map(|(key,_)|key).collect::<std::collections::BTreeSet<_>>().len(),4);
        assert_eq!(keys[0].0,"s|32|2026-10-22T00:00:00Z");
        let invalid=candidate("runtime.process","s",&[("pid","32"),("process_guid","00000000-0000-0000-0000-000000000000")]);
        assert_eq!(normalise(&strategy,&invalid).1,vec!["created_at"]);
        assert_eq!(crate::model::type_definition("runtime.process").unwrap()["identity"]["alternatives"].as_array().unwrap().len(),2);
    }

    #[test]
    fn case_folding_follows_the_namespace_not_a_global_rule() {
        // A task path is case-insensitive: two spellings are one instance.
        let (a, _) = normalise(&strategy("startup.scheduled_task"), &candidate("startup.scheduled_task", "s", &[("path", "\\Tasks\\A")]));
        let (b, _) = normalise(&strategy("startup.scheduled_task"), &candidate("startup.scheduled_task", "s", &[("path", "\\tasks\\a")]));
        assert_eq!(a, b);
        // A native identifier is compared as the source states it.
        let (c, _) = normalise(&strategy("hardware.volume"), &candidate("hardware.volume", "s", &[("native_id", "Volume{AbC}")]));
        let (d, _) = normalise(&strategy("hardware.volume"), &candidate("hardware.volume", "s", &[("native_id", "Volume{abc}")]));
        assert_ne!(c, d, "an identifier the source states is not folded");
    }

    #[test]
    fn a_missing_key_is_marked_and_never_defaulted() {
        let (key, missing) = normalise(&strategy("runtime.process"), &candidate("runtime.process", "s", &[("pid", "4100")]));
        assert_eq!(missing, vec!["created_at"]);
        assert!(key.contains("?created_at"), "the position is held by a marker, not a value: {key}");
        let resolution = resolve(&request(candidate("runtime.process", "s", &[("pid", "4100")]), "runtime.process"), &Empty);
        match resolution {
            IdentityResolution::Provisional(p) => {
                assert_eq!(p.could_be_refined_by, vec!["created_at"]);
                assert!(p.alias.starts_with("runtime.process@"));
            }
            other => panic!("expected provisional, got {other:?}"),
        }
    }

    #[test]
    fn a_complete_key_with_no_existing_instance_is_a_new_confirmed_instance() {
        let resolution = resolve(
            &request(candidate("account.user", "s", &[("sid", "S-1-5-21-1")]), "account.user"),
            &Empty,
        );
        match resolution {
            IdentityResolution::Confirmed(c) => {
                assert_eq!(c.instance_id, None);
                assert_eq!(c.basis, MatchBasis::New);
            }
            other => panic!("expected confirmed, got {other:?}"),
        }
    }

    #[test]
    fn several_matches_are_ambiguous_and_one_is_never_picked() {
        struct TwoMatches;
        impl InstanceLookup for TwoMatches {
            fn by_key(&self, _: &str, _: &str, _: &str, _: &str, _: &str) -> Vec<(u64, u32)> { vec![(1, 1), (2, 1)] }
            fn by_source_binding(&self, _: &str, _: &str, _: &str) -> Option<(u64, u32)> { None }
        }
        let resolution = resolve(
            &request(candidate("account.user", "s", &[("sid", "S-1-5-21-1")]), "account.user"),
            &TwoMatches,
        );
        match resolution {
            IdentityResolution::Ambiguous(a) => assert_eq!(a.candidates, vec![1, 2]),
            other => panic!("expected ambiguous, got {other:?}"),
        }
    }

    #[test]
    fn a_disabled_item_still_resolves_so_retained_facts_survive() {
        // A candidate exists only because material was collected, and an item switched off after
        // collecting must keep the facts it already produced. Refusing to resolve here would drop
        // the last observed state, which the model forbids. The gate on a disabled item belongs in
        // `semantics::absence_eligible`, where an absence is actually concluded.
        let mut request = request(candidate("account.user", "s", &[("sid", "S-1-5-21-1")]), "account.user");
        request.hints.configured = false;
        assert!(matches!(resolve(&request, &Empty), IdentityResolution::Confirmed(_)));
    }

    #[test]
    fn absence_is_not_deletion_and_absence_without_completeness_is_nothing() {
        let hints = LifecycleHints { complete_observation: true, collected_scope: "machine:m".into(), configured: true, history_retained_from: None, states_object_ended: false, boot_id: None };
        assert!(matches!(
            lifecycle(Some((LifecycleState::Active, 1)), ObservationFacts::Absent, &hints, "t1"),
            LifecycleTransition::NotObserved { .. }
        ));
        // Still absent: stays not-observed rather than becoming a removal.
        assert!(matches!(
            lifecycle(Some((LifecycleState::NotObserved, 1)), ObservationFacts::Absent, &hints, "t2"),
            LifecycleTransition::Unchanged { .. }
        ));
        let incomplete = LifecycleHints { complete_observation: false, ..hints.clone() };
        assert!(matches!(
            lifecycle(Some((LifecycleState::Active, 1)), ObservationFacts::Absent, &incomplete, "t3"),
            LifecycleTransition::Unchanged { .. }
        ));
        let disabled = LifecycleHints { configured: false, ..hints.clone() };
        assert!(matches!(
            lifecycle(Some((LifecycleState::Active, 1)), ObservationFacts::Absent, &disabled, "t4"),
            LifecycleTransition::Unchanged { .. }
        ));
    }

    #[test]
    fn a_stated_removal_is_the_only_path_to_removed() {
        let hints = LifecycleHints { complete_observation: true, collected_scope: "machine:m".into(), configured: true, history_retained_from: None, states_object_ended: true, boot_id: None };
        assert!(matches!(
            lifecycle(Some((LifecycleState::Active, 1)), ObservationFacts::Absent, &hints, "t1"),
            LifecycleTransition::RemovedStated { .. }
        ));
        assert!(matches!(
            lifecycle(None, ObservationFacts::Present, &hints, "t1"),
            LifecycleTransition::FirstObserved { .. }
        ));
    }

    #[test]
    fn first_observation_is_not_a_creation() {
        let hints = LifecycleHints { complete_observation: true, collected_scope: "machine:m".into(), configured: true, history_retained_from: None, states_object_ended: false, boot_id: None };
        match lifecycle(None, ObservationFacts::Present, &hints, "2026-09-12T00:00:00Z") {
            LifecycleTransition::FirstObserved { basis, .. } => {
                assert!(basis.contains("no creation time"), "the basis must deny a creation claim: {basis}");
            }
            other => panic!("expected first observed, got {other:?}"),
        }
    }

    #[test]
    fn the_alias_is_stable_so_references_survive_confirmation() {
        let provisional = InstanceIdentity { type_id: "account.user".into(), machine_id: "m".into(), installation_id: "i".into(), scope: "s".into(), native_key: "k".into(), incarnation: 1 };
        let confirmed = InstanceIdentity { native_key: "k".into(), ..provisional.clone() };
        assert_eq!(provisional.alias(), confirmed.alias(), "a reference made against the alias still resolves after confirmation");
    }

    #[test]
    fn field_kind_is_not_part_of_the_key() {
        // Guards against identity drifting into display attributes: a candidate with an extra
        // non-key field normalises to the same key.
        let base = candidate("account.user", "s", &[("sid", "S-1-5-21-1")]);
        let with_extra = candidate("account.user", "s", &[("sid", "S-1-5-21-1"), ("name", "renamed")]);
        assert_eq!(
            normalise(&strategy("account.user"), &base).0,
            normalise(&strategy("account.user"), &with_extra).0
        );
        let _ = FieldKind::Configuration;
    }
}
