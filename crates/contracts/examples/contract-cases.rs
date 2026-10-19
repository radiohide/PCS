//! R06 contract-case runner.
//!
//! Runs each case under `fixtures/contract/` through the reference pipeline — adapter, identity,
//! fact selection, absence eligibility — and writes the *observed* result. It never writes
//! expectations: `case.json` holds hand-authored assertions with their evidence basis, and
//! `scripts/check-contract-cases.cjs` compares them against this output. An expectation can
//! therefore not be produced by the code it is meant to judge.
//!
//! usage: contract-cases [cases-directory] [output-path]
use pcscope_contracts::adapter::*;
use pcscope_contracts::identity::*;
use pcscope_contracts::model;
use pcscope_contracts::reference::{adapter_for, read_fixture};
use pcscope_contracts::semantics::*;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// In-memory lookup, mirroring what S06 will provide from the store.
#[derive(Default)]
struct Lookup {
    instances: Vec<(u64, String, String, String, String, String, u32)>,
    bindings: Vec<(String, String, String, u64)>,
    next_id: u64,
}
impl Lookup {
    fn incarnation_of(&self, instance_id: u64) -> u32 {
        self.instances.iter().find(|(id, ..)| *id == instance_id).map(|(_, _, _, _, _, _, incarnation)| *incarnation).unwrap_or(1)
    }
    fn allocate(&mut self, identity: &InstanceIdentity) -> u64 {
        self.next_id += 1;
        self.instances.push((
            self.next_id,
            identity.type_id.clone(),
            identity.machine_id.clone(),
            identity.installation_id.clone(),
            identity.scope.clone(),
            identity.native_key.clone(),
            identity.incarnation,
        ));
        self.next_id
    }
    fn bind(&mut self, record: &SourceRecordRef, instance_id: u64) {
        self.bindings.push((record.source.clone(), record.item_id.clone(), record.native_record_id.clone(), instance_id));
    }
}
impl InstanceLookup for Lookup {
    fn by_key(&self, type_id: &str, machine_id: &str, installation_id: &str, scope: &str, native_key: &str) -> Vec<(u64, u32)> {
        self.instances
            .iter()
            .filter(|(_, t, m, i, s, k, _)| t == type_id && m == machine_id && i == installation_id && s == scope && k == native_key)
            .map(|(id, _, _, _, _, _, incarnation)| (*id, *incarnation))
            .collect()
    }
    fn by_source_binding(&self, source: &str, item_id: &str, native_record_id: &str) -> Option<(u64, u32)> {
        self.bindings
            .iter()
            .find(|(s, i, n, _)| s == source && i == item_id && n == native_record_id)
            .map(|(_, _, _, id)| (*id, self.incarnation_of(*id)))
    }
}

fn transition_name(transition: &LifecycleTransition) -> &'static str {
    match transition {
        LifecycleTransition::FirstObserved { .. } => "first_observed",
        LifecycleTransition::StillObserved => "still_observed",
        LifecycleTransition::ObservedAgain { .. } => "observed_again",
        LifecycleTransition::NotObserved { .. } => "not_observed",
        LifecycleTransition::EndedStated { .. } => "ended_stated",
        LifecycleTransition::RemovedStated { .. } => "removed_stated",
        LifecycleTransition::Reincarnated { .. } => "reincarnated",
        LifecycleTransition::Unchanged { .. } => "unchanged",
    }
}

fn basis_name(basis: &MatchBasis) -> &'static str {
    match basis {
        MatchBasis::ExistingByKey => "existing_by_key",
        MatchBasis::ExistingBySourceBinding => "existing_by_source_binding",
        MatchBasis::New => "new",
    }
}

fn case_file(directory: &Path, name: &str) -> Value {
    let text = std::fs::read_to_string(directory.join(name)).unwrap_or_else(|error| panic!("cannot read {name}: {error}"));
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("{name} is not valid JSON: {error}"))
}

/// Declared gaps in the input. A gap is a collection-state record, not an object, so the runner
/// reports it separately instead of letting it disappear among the evidence inputs.
fn gap_records(path: &Path) -> Vec<Value> {
    let Ok(text) = std::fs::read_to_string(path) else { return Vec::new() };
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|value| value["type"] == "gap")
        .map(|value| json!({"source": value["source"], "reason": value["reason"], "observed_at": value["observed_at"]}))
        .collect()
}

/// A case may declare that a field could not be read or that its evidence was pruned. Applying it
/// here keeps that scaffolding in the test tool rather than in the reference adapter, which is
/// meant to model a real adapter.
fn apply_field_overrides(observations: &mut [ObservationCandidate], overrides: &serde_json::Map<String, Value>) {
    if overrides.is_empty() {
        return;
    }
    for observation in observations.iter_mut() {
        for field in observation.fields.iter_mut() {
            let Some(status) = overrides.get(&field.field_id).and_then(Value::as_str) else { continue };
            field.status = match status {
                "not_provided" => ValueStatus::NotProvided,
                "unreadable" => ValueStatus::Unreadable,
                "not_applicable" => ValueStatus::NotApplicable,
                "evidence_pruned" => ValueStatus::EvidencePruned,
                other => panic!("unknown field status override: {other}"),
            };
            // Only a status that admits a value keeps one.
            field.value = None;
            field.value_type = "unknown".into();
        }
    }
}

fn run_case(directory: &Path) -> Value {
    let case = case_file(directory, "case.json");
    let name = case["name"].as_str().unwrap_or("unnamed").to_owned();
    let configured = case["configured"].as_bool().unwrap_or(true);
    let complete = case["complete"].as_bool().unwrap_or(true);
    let collected_scope = case["collected_scope"].as_str().unwrap_or("install:example").to_owned();
    let machine_id = case["machine_id"].as_str().unwrap_or("example-machine").to_owned();
    let installation_id = case["installation_id"].as_str().unwrap_or("example-install").to_owned();
    // A case may state that a source itself declared the object gone; that is the only route to a
    // removal, so it is a run parameter rather than something inferred from absence.
    let states_object_ended = case["states_object_ended"].as_bool().unwrap_or(false);

    let inputs = read_fixture(&directory.join("input.jsonl")).unwrap_or_else(|error| panic!("{name}: {error}"));
    let gaps = gap_records(&directory.join("input.jsonl"));
    let overrides = case["field_status_overrides"].as_object().cloned().unwrap_or_default();
    let context = AdapterContext {
        catalog_version: model::catalog_version().into(),
        rule_version: model::catalog_fingerprint(),
        machine_id: machine_id.clone(),
        installation_id: installation_id.clone(),
        boot_id: None,
        collected_scope: collected_scope.clone(),
        complete,
    };
    let hints = LifecycleHints {
        complete_observation: complete,
        collected_scope: collected_scope.clone(),
        configured,
        history_retained_from: None,
        states_object_ended,
        boot_id: None,
    };

    let mut lookup = Lookup::default();
    let mut lifecycle: BTreeMap<u64, LifecycleState> = BTreeMap::new();
    let mut transitions: Vec<Value> = Vec::new();
    let mut instances: Vec<Value> = Vec::new();
    let mut actions: Vec<Value> = Vec::new();
    let mut problems: Vec<Value> = Vec::new();
    // field_id -> (instance alias, observations)
    let mut field_observations: BTreeMap<(String, String), Vec<FactObservation>> = BTreeMap::new();

    for input in &inputs {
        let Some(adapter) = adapter_for(&input.source) else {
            problems.push(json!({"source": input.source, "problem": "no reference adapter for this source"}));
            continue;
        };
        let mut result = adapter.adapt(input, &context);
        apply_field_overrides(&mut result.observations, &overrides);
        for problem in &result.problems {
            problems.push(json!({"source": input.source, "problem": problem.message}));
        }
        for observation in &result.observations {
            let request = IdentityRequest {
                candidate: observation.identity.clone(),
                strategy: TypeStrategy::for_type(&observation.identity.type_id).expect("declared type"),
                binding: SourceRecordRef { source: input.source.clone(), item_id: input.item_id.clone(), native_record_id: input.native_record_id.clone() },
                machine_id: machine_id.clone(),
                installation_id: installation_id.clone(),
                evidence_time: EvidenceTime { source_time: input.source_time.clone(), observed_at: input.observed_at.clone() },
                hints: hints.clone(),
                rule_version: model::catalog_fingerprint(),
            };
            let resolution = resolve(&request, &lookup);
            let (alias, status, id) = record_instance(&mut lookup, &mut lifecycle, &mut transitions, &resolution, &request);
            if !instances.iter().any(|entry| entry["alias"] == json!(alias)) {
                instances.push(json!({
                    "alias": alias, "type_id": observation.identity.type_id,
                    "scope": observation.identity.scope, "identity_status": status,
                    "instance_id": id,
                    "outcome": match &resolution {
                        IdentityResolution::Confirmed(c) => format!("confirmed:{}", basis_name(&c.basis)),
                        IdentityResolution::Provisional(_) => "provisional".into(),
                        IdentityResolution::Ambiguous(_) => "ambiguous".into(),
                        IdentityResolution::Unresolved { .. } => "unresolved".into(),
                    },
                    "missing_keys": match &resolution {
                        IdentityResolution::Provisional(p) => p.could_be_refined_by.clone(),
                        _ => Vec::new(),
                    },
                }));
            }
            for field in &observation.fields {
                field_observations
                    .entry((alias.clone(), field.field_id.clone()))
                    .or_default()
                    .push(FactObservation {
                        source: input.source.clone(),
                        value: field.value.clone(),
                        value_type: field.value_type.clone(),
                        status: field.status,
                        source_time: field.source_time.clone().or_else(|| input.source_time.clone()),
                        observed_at: input.observed_at.clone(),
                        evidence: observation.evidence.clone(),
                    });
            }
        }
        for action in &result.actions {
            let (target_alias, status, target_id) = match &action.target {
                ActionTarget::Assigned { identity } => {
                    let request = IdentityRequest {
                        candidate: identity.clone(),
                        strategy: TypeStrategy::for_type(&identity.type_id).expect("declared type"),
                        binding: SourceRecordRef { source: input.source.clone(), item_id: input.item_id.clone(), native_record_id: input.native_record_id.clone() },
                        machine_id: machine_id.clone(),
                        installation_id: installation_id.clone(),
                        evidence_time: EvidenceTime { source_time: input.source_time.clone(), observed_at: input.observed_at.clone() },
                        hints: hints.clone(),
                        rule_version: model::catalog_fingerprint(),
                    };
                    let resolution = resolve(&request, &lookup);
                    let (alias, status, id) = record_instance(&mut lookup, &mut lifecycle, &mut transitions, &resolution, &request);
                    if !instances.iter().any(|entry| entry["alias"] == json!(alias)) {
                        instances.push(json!({
                            "alias": alias, "type_id": identity.type_id, "scope": identity.scope,
                            "identity_status": status, "instance_id": id,
                            "outcome": match &resolution {
                                IdentityResolution::Confirmed(c) => format!("confirmed:{}", basis_name(&c.basis)),
                                IdentityResolution::Provisional(_) => "provisional".into(),
                                IdentityResolution::Ambiguous(_) => "ambiguous".into(),
                                IdentityResolution::Unresolved { .. } => "unresolved".into(),
                            },
                            "missing_keys": match &resolution {
                                IdentityResolution::Provisional(p) => p.could_be_refined_by.clone(),
                                _ => Vec::new(),
                            },
                        }));
                    }
                    (Some(alias), status, id)
                }
                ActionTarget::Unassigned { .. } => (None, "unassigned".to_string(), None),
            };
            actions.push(json!({
                "action_id": action.action_id,
                "target_alias": target_alias,
                "target_status": status,
                "result": action.result,
                "phase": action.phase,
                "precision": action.precision,
                "occurred_at": action.occurred_at,
                "interval_from": action.interval_from,
                "interval_until": action.interval_until,
                "unassigned_reason": match &action.target { ActionTarget::Unassigned { reason } => Some(reason.clone()), _ => None },
                "evidence_count": action.evidence.len(),
                "source": input.source,
            }));
            // Only a source-stated removal moves the instance to a terminal state. This is what
            // makes a later reappearance a new generation rather than a continuation.
            if STATED_REMOVALS.contains(&action.action_id.as_str()) {
                if let Some(instance_id) = target_id {
                    lifecycle.insert(instance_id, LifecycleState::Removed);
                    transitions.push(json!({
                        "instance": target_alias,
                        "kind": "removed_stated", "detail": {"transition": "removed_stated", "basis": "来源明确声明对象已删除"},
                    }));
                }
            }
        }
    }

    let mut current_values: Vec<Value> = Vec::new();
    for ((alias, field_id), observations) in &field_observations {
        let selection = select_current(observations);
        current_values.push(json!({
            "instance_alias": alias,
            "field_id": field_id,
            "observations": observations.len(),
            "selection": serde_json::to_value(&selection).unwrap(),
        }));
    }

    // Absence eligibility per source present in the input.
    let mut sources: Vec<String> = inputs.iter().map(|input| input.source.clone()).collect();
    sources.sort();
    sources.dedup();
    let coverage: Vec<Value> = sources
        .iter()
        .map(|source| {
            let verdict = absence_eligible(&AbsenceFacts {
                snapshot_complete: complete,
                item_configured: configured,
                collection_succeeded: true,
                instance_scope_covered: true,
            });
            json!({"source": source, "verdict": serde_json::to_value(&verdict).unwrap()})
        })
        .collect();

    json!({
        "case": name,
        "input_count": inputs.len(),
        "instances": instances,
        "actions": actions,
        "current_values": current_values,
        "coverage": coverage,
        "transitions": transitions,
        "gaps": gaps,
        "problems": problems,
    })
}

/// Actions whose record states the object itself was removed. Only a stated removal moves an
/// instance to a terminal state; absence never does (R04, absence eligibility).
const STATED_REMOVALS: [&str; 3] = ["startup.task_deleted", "identity.account_deleted", "files.deleted"];

fn record_instance(
    lookup: &mut Lookup,
    states: &mut BTreeMap<u64, LifecycleState>,
    transitions: &mut Vec<Value>,
    resolution: &IdentityResolution,
    request: &IdentityRequest,
) -> (String, String, Option<u64>) {
    let terminal = |state: Option<&LifecycleState>| matches!(state, Some(LifecycleState::Ended) | Some(LifecycleState::Removed));
    match resolution {
        IdentityResolution::Confirmed(confirmed) => {
            // The identity rule says a key that reappears after a stated removal is a new generation,
            // not a continuation. The runner applies that rule here; full lifecycle integration with
            // absence across rounds is S06.
            if let Some(existing) = confirmed.instance_id {
                if terminal(states.get(&existing)) {
                    let previous = *states.get(&existing).unwrap_or(&LifecycleState::Removed);
                    let transition = lifecycle(
                        Some((previous, confirmed.identity.incarnation)),
                        ObservationFacts::Present,
                        &request.hints,
                        &request.evidence_time.observed_at,
                    );
                    let incarnation = match &transition {
                        LifecycleTransition::Reincarnated { incarnation, .. } => *incarnation,
                        _ => confirmed.identity.incarnation.saturating_add(1),
                    };
                    let identity = InstanceIdentity { incarnation, ..confirmed.identity.clone() };
                    let id = lookup.allocate(&identity);
                    lookup.bind(&request.binding, id);
                    states.insert(id, LifecycleState::Active);
                    transitions.push(json!({"instance": identity.alias(), "kind": transition_name(&transition), "detail": serde_json::to_value(&transition).unwrap()}));
                    return (identity.alias(), "confirmed".into(), Some(id));
                }
            }
            let id = confirmed.instance_id.unwrap_or_else(|| lookup.allocate(&confirmed.identity));
            lookup.bind(&request.binding, id);
            states.entry(id).or_insert(LifecycleState::Active);
            (confirmed.identity.alias(), "confirmed".into(), Some(id))
        }
        IdentityResolution::Provisional(provisional) => {
            let id = provisional.instance_id.unwrap_or_else(|| lookup.allocate(&provisional.identity));
            lookup.bind(&request.binding, id);
            states.entry(id).or_insert(LifecycleState::Active);
            (provisional.alias.clone(), "provisional".into(), Some(id))
        }
        IdentityResolution::Ambiguous(ambiguity) => (ambiguity.identity.alias(), "ambiguous".into(), None),
        IdentityResolution::Unresolved { .. } => (String::new(), "unresolved".into(), None),
    }
}

fn main() {
    let cases: PathBuf = std::env::args().nth(1).unwrap_or_else(|| "fixtures/contract".into()).into();
    let output: PathBuf = std::env::args().nth(2).unwrap_or_else(|| "model/contract-observed.json".into()).into();
    let mut directories: Vec<PathBuf> = std::fs::read_dir(&cases)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", cases.display()))
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_dir())
        .collect();
    directories.sort();
    let results: Vec<Value> = directories.iter().map(|directory| run_case(directory)).collect();
    let document = json!({
        "generated_by": "crates/contracts/examples/contract-cases.rs",
        "note": "契约用例的实测结果。断言在 fixtures/contract/*/case.json 中手写，本工具不生成断言。",
        "catalog_version": model::catalog_version(),
        "rule_version": model::catalog_fingerprint(),
        "cases": results,
    });
    std::fs::write(&output, serde_json::to_string_pretty(&document).unwrap() + "\n").expect("write observed");
    println!("contract cases: {} run, written to {}", results.len(), output.display());
    for case in &results {
        println!(
            "  {}: {} input, {} instances, {} actions",
            case["case"].as_str().unwrap_or("?"),
            case["input_count"],
            case["instances"].as_array().map_or(0, Vec::len),
            case["actions"].as_array().map_or(0, Vec::len)
        );
    }
}
