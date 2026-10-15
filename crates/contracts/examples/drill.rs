//! R09 minimal end-to-end drill.
//!
//! Runs the contract cases through the whole chain with the real DTOs — evidence input, adapter
//! candidates, identity resolution, fact selection, a minimal projection, and a query response of
//! the shape the frontend consumes — and writes the responses.
//!
//! The projection here is deliberately minimal and is a **stand-in**: instances, facts and actions
//! are built in memory from the pipeline result, not written to or read from the store. S05–S08
//! replace it. Nothing in this file claims those steps are done.
//!
//! usage: drill [cases-directory] [output-path]
use pcscope_contracts::adapter::*;
use pcscope_contracts::identity::*;
use pcscope_contracts::model;
use pcscope_contracts::query::*;
use pcscope_contracts::reference::{adapter_for, read_fixture};
use pcscope_contracts::semantics::*;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// In-memory stand-in for the store S06 will provide.
#[derive(Default)]
struct Lookup {
    instances: Vec<(u64, InstanceIdentity, LifecycleState)>,
    bindings: Vec<(String, String, String, u64)>,
    next_id: u64,
}
impl Lookup {
    fn allocate(&mut self, identity: &InstanceIdentity) -> u64 {
        self.next_id += 1;
        self.instances.push((self.next_id, identity.clone(), LifecycleState::Active));
        self.next_id
    }
    fn state_of(&self, id: u64) -> Option<LifecycleState> {
        self.instances.iter().find(|(candidate, ..)| *candidate == id).map(|(_, _, state)| *state)
    }
    fn set_state(&mut self, id: u64, state: LifecycleState) {
        if let Some(entry) = self.instances.iter_mut().find(|(candidate, ..)| *candidate == id) {
            entry.2 = state;
        }
    }
}
impl InstanceLookup for Lookup {
    fn by_key(&self, type_id: &str, machine_id: &str, installation_id: &str, scope: &str, native_key: &str) -> Vec<(u64, u32)> {
        self.instances
            .iter()
            .filter(|(_, identity, _)| identity.type_id == type_id && identity.machine_id == machine_id && identity.installation_id == installation_id && identity.scope == scope && identity.native_key == native_key)
            .map(|(id, identity, _)| (*id, identity.incarnation))
            .collect()
    }
    fn by_source_binding(&self, source: &str, item_id: &str, native_record_id: &str) -> Option<(u64, u32)> {
        self.bindings
            .iter()
            .find(|(s, i, n, _)| s == source && i == item_id && n == native_record_id)
            .map(|(_, _, _, id)| (*id, self.instances.iter().find(|(candidate, ..)| candidate == id).map(|(_, identity, _)| identity.incarnation).unwrap_or(1)))
    }
}

/// The minimal projection: everything the frontend needs for one case, built in memory.
#[derive(Default)]
struct Projection {
    summaries: BTreeMap<String, InstanceSummary>,
    fields: BTreeMap<String, Vec<FieldView>>,
    actions: Vec<ActionView>,
    evidence: BTreeMap<String, EvidenceView>,
    counts: Counts,
}

fn field_kind_name(kind: FieldKind) -> &'static str {
    match kind {
        FieldKind::Identity => "identity",
        FieldKind::Configuration => "configuration",
        FieldKind::State => "state",
    }
}
fn status_name(status: ValueStatus) -> &'static str {
    match status {
        ValueStatus::Known => "known",
        ValueStatus::NotProvided => "not_provided",
        ValueStatus::Unreadable => "unreadable",
        ValueStatus::NotApplicable => "not_applicable",
        ValueStatus::Conflicting => "conflicting",
        ValueStatus::Masked => "masked",
        ValueStatus::EvidencePruned => "evidence_pruned",
    }
}
fn derivation_name(derivation: Derivation) -> &'static str {
    match derivation {
        Derivation::SourceProvided => "source_provided",
        Derivation::Decoded => "decoded",
        Derivation::SnapshotComparison => "snapshot_comparison",
        Derivation::Rule => "rule",
    }
}
fn precision_name(precision: TimePrecision) -> &'static str {
    match precision {
        TimePrecision::Exact => "exact",
        TimePrecision::Interval => "interval",
        TimePrecision::ObservationOnly => "observation_only",
    }
}
fn outcome_name(outcome: ActionOutcome) -> &'static str {
    match outcome {
        ActionOutcome::Succeeded => "succeeded",
        ActionOutcome::Failed => "failed",
        ActionOutcome::Started => "started",
        ActionOutcome::Ended => "ended",
        ActionOutcome::Blocked => "blocked",
        ActionOutcome::Observed => "observed",
        ActionOutcome::Unknown => "unknown",
    }
}
/// Label and kind come from the catalog. A field with no declaration is a contract violation
/// already caught by `adapter::validate`; here it falls back to the ID rather than inventing a name.
fn label_for(type_id: &str, field_id: &str) -> String {
    model::field_definition(type_id, field_id).map(|(label, _)| label.to_owned()).unwrap_or_else(|| field_id.to_owned())
}
fn kind_for(type_id: &str, field_id: &str, declared: FieldKind) -> String {
    model::field_definition(type_id, field_id).map(|(_, kind)| kind.to_owned()).unwrap_or_else(|| field_kind_name(declared).to_owned())
}

/// A readable name for the instance list. An identity field is used when it is one a person would
/// recognise; otherwise the identity key stands in, so the row is never blank. A display name never
/// participates in the identity itself.
fn display_name_for(candidate: &IdentityCandidate) -> String {
    candidate
        .fields
        .iter()
        .find(|field| matches!(field.key.as_str(), "name" | "path" | "device_id" | "vm_id" | "native_id"))
        .map(|field| field.value.clone())
        .unwrap_or_else(|| candidate.fields.iter().map(|field| field.value.clone()).collect::<Vec<_>>().join("|"))
}

fn evidence_view(reference: &EvidenceRef, source: &str, observed_at: &str, pruned: bool) -> EvidenceView {
    EvidenceView {
        reference: reference.clone(),
        state: if pruned { "pruned" } else { "available" }.into(),
        pruned_at: pruned.then(|| "2026-09-12T00:00:00.000Z".to_owned()),
        retained_from: pruned.then(|| "2026-09-01T00:00:00.000Z".to_owned()),
        source: source.into(),
        source_time: None,
        observed_at: observed_at.into(),
        ingested_at: observed_at.into(),
        decoding: "available".into(),
        problems: if pruned { vec!["原始材料已按保留策略清理".into()] } else { Vec::new() },
        raw_available: !pruned,
    }
}

fn project(directory: &Path) -> anyhow::Result<Value> {
    let case: Value = serde_json::from_str(&std::fs::read_to_string(directory.join("case.json"))?)?;
    let name = case["name"].as_str().unwrap_or("unnamed").to_owned();
    let machine_id = case["machine_id"].as_str().unwrap_or("example-machine").to_owned();
    let installation_id = case["installation_id"].as_str().unwrap_or("example-install").to_owned();
    let collected_scope = case["collected_scope"].as_str().unwrap_or("install:example").to_owned();
    let configured = case["configured"].as_bool().unwrap_or(true);
    let complete = case["complete"].as_bool().unwrap_or(true);
    let overrides = case["field_status_overrides"].as_object().cloned().unwrap_or_default();
    // A case may declare that a reference's evidence was pruned; the drill then reports the
    // reference as pruned rather than dropping it.
    let pruned_evidence = case["pruned_evidence"].as_bool().unwrap_or(false);

    let inputs = read_fixture(&directory.join("input.jsonl"))?;
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
        states_object_ended: case["states_object_ended"].as_bool().unwrap_or(false),
        boot_id: None,
    };

    let mut lookup = Lookup::default();
    let mut projection = Projection::default();
    let mut field_observations: BTreeMap<(String, String), Vec<FactObservation>> = BTreeMap::new();
    let mut field_declarations: BTreeMap<(String, String), FieldKind> = BTreeMap::new();
    let mut list: Vec<InstanceSummary> = Vec::new();

    let resolve_one = |lookup: &mut Lookup,
                       projection: &mut Projection,
                       candidate: &IdentityCandidate,
                       binding: SourceRecordRef,
                       source_time: Option<String>,
                       observed_at: &str|
     -> (String, String, Option<u64>) {
        let request = IdentityRequest {
            candidate: candidate.clone(),
            strategy: TypeStrategy::for_type(&candidate.type_id).expect("declared type"),
            binding: binding.clone(),
            machine_id: machine_id.clone(),
            installation_id: installation_id.clone(),
            evidence_time: EvidenceTime { source_time, observed_at: observed_at.to_owned() },
            hints: hints.clone(),
            rule_version: model::catalog_fingerprint(),
        };
        let resolution = resolve(&request, lookup);
        let (alias, status, id) = match &resolution {
            IdentityResolution::Confirmed(confirmed) => {
                // A key that reappears after a stated removal is a new generation.
                let terminal = confirmed.instance_id.is_some_and(|existing| matches!(lookup.state_of(existing), Some(LifecycleState::Ended) | Some(LifecycleState::Removed)));
                if terminal {
                    let identity = InstanceIdentity { incarnation: confirmed.identity.incarnation.saturating_add(1), ..confirmed.identity.clone() };
                    let id = lookup.allocate(&identity);
                    lookup.bindings.push((binding.source.clone(), binding.item_id.clone(), binding.native_record_id.clone(), id));
                    (identity.alias(), "confirmed".to_owned(), Some(id))
                } else {
                    let id = confirmed.instance_id.unwrap_or_else(|| lookup.allocate(&confirmed.identity));
                    lookup.bindings.push((binding.source.clone(), binding.item_id.clone(), binding.native_record_id.clone(), id));
                    (confirmed.identity.alias(), "confirmed".to_owned(), Some(id))
                }
            }
            IdentityResolution::Provisional(provisional) => {
                let id = provisional.instance_id.unwrap_or_else(|| lookup.allocate(&provisional.identity));
                lookup.bindings.push((binding.source.clone(), binding.item_id.clone(), binding.native_record_id.clone(), id));
                (provisional.alias.clone(), "provisional".to_owned(), Some(id))
            }
            IdentityResolution::Ambiguous(ambiguity) => (ambiguity.identity.alias(), "ambiguous".to_owned(), None),
            IdentityResolution::Unresolved { .. } => (String::new(), "unresolved".to_owned(), None),
        };
        if !projection.summaries.contains_key(&alias) && !alias.is_empty() {
            if let Some((category, type_name, _)) = model::type_summary(&candidate.type_id) {
                projection.summaries.insert(alias.clone(), InstanceSummary {
                    instance_id: id,
                    alias: alias.clone(),
                    type_id: candidate.type_id.clone(),
                    category: category.into(),
                    type_name: type_name.into(),
                    record_basis: None,
                    presentation: None,
                    display_name: display_name_for(candidate),
                    scope: candidate.scope.clone(),
                    identity_status: status.clone(),
                    lifecycle: "active".into(),
                    key_facts: Vec::new(),
                    changed_at: None,
                    seen_at: Some(observed_at.to_owned()),
                    coverage: Vec::new(),
                });
            }
        }
        (alias, status, id)
    };

    let mut problems: Vec<Value> = Vec::new();
    for input in &inputs {
        let Some(adapter) = adapter_for(&input.source) else {
            problems.push(json!({"source": input.source, "problem": "no reference adapter for this source"}));
            continue;
        };
        let mut result = adapter.adapt(input, &context);
        for problem in &result.problems {
            problems.push(json!({"source": input.source, "problem": problem.message}));
        }
        // The case may state that a field could not be read or that its evidence was pruned.
        for observation in result.observations.iter_mut() {
            for field in observation.fields.iter_mut() {
                let Some(status) = overrides.get(&field.field_id).and_then(Value::as_str) else { continue };
                field.status = match status {
                    "unreadable" => ValueStatus::Unreadable,
                    "not_provided" => ValueStatus::NotProvided,
                    "evidence_pruned" => ValueStatus::EvidencePruned,
                    other => panic!("unknown override {other}"),
                };
                field.value = None;
                field.value_type = "unknown".into();
            }
        }
        for observation in &result.observations {
            let binding = SourceRecordRef { source: input.source.clone(), item_id: input.item_id.clone(), native_record_id: input.native_record_id.clone() };
            let (alias, _, id) = resolve_one(&mut lookup, &mut projection, &observation.identity, binding, input.source_time.clone(), &input.observed_at);
            if pruned_evidence {
                projection.evidence.insert(input.evidence.id.clone(), evidence_view(&input.evidence, &input.source, &input.observed_at, true));
            } else {
                projection.evidence.entry(input.evidence.id.clone()).or_insert_with(|| evidence_view(&input.evidence, &input.source, &input.observed_at, false));
            }
            for field in &observation.fields {
                field_declarations.insert((alias.clone(), field.field_id.clone()), field.field_kind);
                field_observations.entry((alias.clone(), field.field_id.clone())).or_default().push(FactObservation {
                    source: input.source.clone(),
                    value: field.value.clone(),
                    value_type: field.value_type.clone(),
                    status: field.status,
                    source_time: field.source_time.clone().or_else(|| input.source_time.clone()),
                    observed_at: input.observed_at.clone(),
                    evidence: observation.evidence.clone(),
                });
            }
            let _ = id;
        }
        for action in &result.actions {
            let (target_alias, unassigned) = match &action.target {
                ActionTarget::Assigned { identity } => {
                    let binding = SourceRecordRef { source: input.source.clone(), item_id: input.item_id.clone(), native_record_id: input.native_record_id.clone() };
                    let (alias, _, _) = resolve_one(&mut lookup, &mut projection, identity, binding, input.source_time.clone(), &input.observed_at);
                    (Some(alias), None)
                }
                ActionTarget::Unassigned { reason } => (None, Some(reason.clone())),
            };
            let title = model::action_definition(&action.action_id).and_then(|definition| definition["name"].as_str()).unwrap_or(&action.action_id).to_owned();
            // A target identified only by its scope carries no distinguishing value, so the action
            // cannot be attached to a meaningful instance. It is kept as unassigned with the reason,
            // which is what the contract provides that state for.
            let unattributable = match &action.target {
                ActionTarget::Assigned { identity } => identity.fields.iter().all(|field| field.key == "scope"),
                ActionTarget::Unassigned { .. } => false,
            };
            if unattributable {
                let reason = format!(
                    "{} 只提供了作用域，没有任何可用于识别目标的值",
                    input.source
                );
                projection.actions.push(ActionView {
                    context: None,
                    source_record: action.source_record.clone(),
                    action_id: action.action_id.clone(),
                    title,
                    result: outcome_name(action.result).into(),
                    phase: action.phase.clone(),
                    precision: precision_name(action.precision).into(),
                    occurred_at: action.occurred_at.clone(),
                    interval_from: action.interval_from.clone(),
                    interval_until: action.interval_until.clone(),
                    observed_at: action.observed_at.clone(),
                    field_id: action.field_id.clone(),
                    before: action.before.clone(),
                    after: action.after.clone(),
                    time_derived: action.precision != TimePrecision::Exact || action.occurred_at.is_none(),
                    evidence: action.evidence.clone(),
                    unassigned_reason: Some(reason),
                });
                projection.counts.source_records += 1;
                continue;
            }
            projection.actions.push(ActionView {
                context: None,
                source_record: action.source_record.clone(),
                action_id: action.action_id.clone(),
                title,
                result: outcome_name(action.result).into(),
                phase: action.phase.clone(),
                precision: precision_name(action.precision).into(),
                occurred_at: action.occurred_at.clone(),
                interval_from: action.interval_from.clone(),
                interval_until: action.interval_until.clone(),
                observed_at: action.observed_at.clone(),
                field_id: action.field_id.clone(),
                before: action.before.clone(),
                after: action.after.clone(),
                // A time the source did not state is marked, so the UI never implies precision it
                // does not have.
                time_derived: action.precision != TimePrecision::Exact || action.occurred_at.is_none(),
                evidence: action.evidence.clone(),
                unassigned_reason: unassigned,
            });
            if let Some(alias) = &target_alias {
                if let Some(summary) = projection.summaries.get_mut(alias) {
                    summary.changed_at = summary.changed_at.clone().or_else(|| Some(action.observed_at.clone()));
                }
            }
            if matches!(action.action_id.as_str(), "startup.task_deleted" | "identity.account_deleted" | "files.deleted") {
                if let Some(id) = lookup.bindings.iter().rev().find(|(s, i, n, _)| s == &input.source && i == &input.item_id && n == &input.native_record_id).map(|(_, _, _, id)| *id) {
                    lookup.set_state(id, LifecycleState::Removed);
                }
            }
        }
        projection.counts.source_records += 1;
        let cited = result.actions.iter().map(|action| action.evidence.len()).sum::<usize>()
            + result.observations.iter().map(|observation| observation.evidence.len()).sum::<usize>();
        projection.counts.evidence += cited as u64;
    }

    // A field observed twice with different values is a configuration or state change. The interval
    // is the two observation times, never a claimed exact moment: no source recorded the edit.
    for ((alias, field_id), observations) in &field_observations {
        // Only observations that state a value take part. Comparing an absence against a value
        // reports a change no source observed — which is what two fixtures did here, because their
        // sources describe one task through different fields, and each source is silent about the
        // fields the other carries. That reading produced four phantom actions for case 01.
        let mut valued: Vec<&FactObservation> = observations.iter().filter(|o| states_a_value(o)).collect();
        if valued.len() < 2 {
            continue;
        }
        valued.sort_by_key(|observation| effective_time(observation));
        let first = *valued.first().unwrap();
        let last = *valued.last().unwrap();
        // Two observations that take effect at the same time are §3's 「生效时间相同」case: the
        // sources agree (one value, all sources cited) or they conflict. Neither is a change over
        // time, and an interval of zero length would claim exactly the change that did not happen.
        if effective_time(first) == effective_time(last) {
            continue;
        }
        if first.value == last.value {
            continue;
        }
        let declared = *field_declarations.get(&(alias.clone(), field_id.clone())).unwrap_or(&FieldKind::Configuration);
        let Some(action_id) = change_action(declared) else { continue };
        let title = model::action_definition(action_id).and_then(|definition| definition["name"].as_str()).unwrap_or(action_id).to_owned();
        projection.actions.push(ActionView {
            context: None,
            source_record: None,
            action_id: action_id.into(),
            title,
            result: outcome_name(ActionOutcome::Observed).into(),
            phase: None,
            precision: precision_name(TimePrecision::Interval).into(),
            occurred_at: None,
            interval_from: Some(first.observed_at.clone()),
            interval_until: Some(last.observed_at.clone()),
            observed_at: last.observed_at.clone(),
            field_id: Some(field_id.clone()),
            before: first.value.clone(),
            after: last.value.clone(),
            // The interval end is a bound, not a stated moment.
            time_derived: true,
            evidence: valued.iter().flat_map(|observation| observation.evidence.clone()).collect(),
            unassigned_reason: None,
        });
    }

    // Current values, then the field views the frontend renders.
    for ((alias, field_id), observations) in &field_observations {
        let selection = select_current(observations);
        let declared = *field_declarations.get(&(alias.clone(), field_id.clone())).unwrap_or(&FieldKind::Configuration);
        let (value, status, evidence, conflicts, source_time) = match &selection {
            CurrentValue::Known { value, effective_time, .. } => (Some(value.clone()), "known", observations.iter().flat_map(|o| o.evidence.clone()).collect(), Vec::new(), Some(effective_time.clone())),
            CurrentValue::Conflict { candidates, .. } => (
                candidates.first().and_then(|candidate| candidate.value.clone()),
                "conflicting",
                candidates.iter().flat_map(|candidate| candidate.evidence.clone()).collect(),
                candidates.iter().map(|candidate| ConflictView { value: candidate.value.clone().unwrap_or(Value::Null), source: candidate.source.clone(), source_time: candidate.source_time.clone() }).collect(),
                None,
            ),
            CurrentValue::Absent { status, .. } => (None, status_name(*status), observations.iter().flat_map(|o| o.evidence.clone()).collect(), Vec::new(), None),
            CurrentValue::Refused { .. } => (None, "not_applicable", Vec::new(), Vec::new(), None),
        };
        let type_id = projection.summaries.get(alias).map(|summary| summary.type_id.clone()).unwrap_or_default();
        let view = FieldView {
            field_id: field_id.clone(),
            label: label_for(&type_id, field_id),
            kind: kind_for(&type_id, field_id, declared),
            value,
            value_type: observations.first().map(|observation| observation.value_type.clone()).unwrap_or_else(|| "unknown".into()),
            status: status.to_owned(),
            source_time,
            observed_at: observations.iter().map(|observation| observation.observed_at.clone()).max().unwrap_or_default(),
            derivation: derivation_name(Derivation::SourceProvided).into(),
            evidence,
            conflicts,
        };
        projection.fields.entry(alias.clone()).or_default().push(view);
    }

    // A field whose status is `evidence_pruned` means the material supporting it was removed by
    // retention. The evidence view for those references must say so rather than reporting them as
    // available, so the conclusion and its missing support are visible together.
    let pruned_references: Vec<String> = projection
        .fields
        .values()
        .flatten()
        .filter(|field| field.status == "evidence_pruned")
        .flat_map(|field| field.evidence.iter().map(|reference| reference.id.clone()))
        .collect();
    for reference in pruned_references {
        if let Some(view) = projection.evidence.get_mut(&reference) {
            *view = evidence_view(&view.reference, &view.source, &view.observed_at, true);
        }
    }

    // Assemble the summaries' key facts and coverage, then build the responses.
    for (alias, summary) in projection.summaries.iter_mut() {
        let mut facts = projection.fields.get(alias).cloned().unwrap_or_default();
        // Key facts: identity first, then the rest, bounded so a summary is not the whole detail.
        facts.sort_by_key(|field| if field.kind == "identity" { 0 } else { 1 });
        facts.truncate(6);
        summary.key_facts = facts;
        summary.coverage = vec![CoverageView {
            source: "pcscope.example".into(),
            scope: collected_scope.clone(),
            configured,
            last_result: if configured { "success".into() } else { "never".into() },
            last_success_at: configured.then(|| "2026-09-12T09:00:00.000Z".into()),
            last_attempt_at: Some("2026-09-12T09:00:00.000Z".into()),
            history_state: "caught_up".into(),
            detail: None,
            // The stand-in projects synchronously as it walks the cases, so it has no queue to
            // report. Zeros are what it actually has, not a value it declined to fill in.
            projection: Default::default(),
        }];
        if let Some(id) = lookup.instances.iter().find(|(_, identity, _)| identity.alias() == *alias).map(|(_, _, state)| state) {
            summary.lifecycle = match id {
                LifecycleState::Active => "active",
                LifecycleState::NotObserved => "not_observed",
                LifecycleState::Ended => "ended",
                LifecycleState::Removed => "removed",
            }
            .into();
        }
    }
    list.extend(projection.summaries.values().cloned());
    projection.counts.instances = list.len() as u64;
    projection.counts.actions = projection.actions.len() as u64;

    let lifecycle_filter = list.iter().map(|summary| summary.lifecycle.clone()).collect::<Vec<_>>();
    let detail = list.first().map(|summary| InstanceDetail {
        summary: summary.clone(),
        fields: projection.fields.get(&summary.alias).cloned().unwrap_or_default(),
        actions_preview: projection.actions.clone(),
        bindings: Vec::new(),
    });

    let watermark = format!("drill-{name}-{}", model::catalog_fingerprint());
    let page = Page { next_cursor: None, limit: DEFAULT_PAGE, watermark, has_more: false };
    // Material that produced no candidate is surfaced as a warning rather than disappearing.
    let warnings: Vec<Warning> = problems
        .iter()
        .map(|problem| Warning {
            code: "material_not_attributed".into(),
            message: problem["problem"].as_str().unwrap_or_default().to_owned(),
            at: problem["source"].as_str().map(str::to_owned),
        })
        .collect();

    let mut responses = Vec::new();
    responses.push(json!({"query": "instance_list", "response": QueryResponse {
        contract_version: 1, catalog_version: model::catalog_version().into(),
        status: if list.is_empty() { ResponseStatus::Empty } else { ResponseStatus::Loaded },
        data: Some(serde_json::to_value(&list)?), warnings: warnings.clone(),
        page: Some(page.clone()), counts: Some(projection.counts.clone()), mock: false,
    }}));
    if let Some(detail) = &detail {
        responses.push(json!({"query": "instance_detail", "response": QueryResponse {
            contract_version: 1, catalog_version: model::catalog_version().into(),
            status: ResponseStatus::Loaded,
            data: Some(serde_json::to_value(detail)?), warnings: Vec::new(),
            page: None, counts: Some(projection.counts.clone()), mock: false,
        }}));
    }
    responses.push(json!({"query": "instance_actions", "response": QueryResponse {
        contract_version: 1, catalog_version: model::catalog_version().into(),
        status: if projection.actions.is_empty() { ResponseStatus::Empty } else { ResponseStatus::Loaded },
        data: Some(serde_json::to_value(&projection.actions)?), warnings: Vec::new(),
        page: Some(page), counts: Some(projection.counts.clone()), mock: false,
    }}));
    for view in projection.evidence.values() {
        responses.push(json!({"query": "evidence_detail", "response": QueryResponse {
            contract_version: 1, catalog_version: model::catalog_version().into(),
            status: if view.state == "pruned" { ResponseStatus::EvidencePruned } else { ResponseStatus::Loaded },
            data: Some(serde_json::to_value(view)?), warnings: Vec::new(),
            page: None, counts: None, mock: false,
        }}));
    }

    // The per-case counts are written out rather than left to be recounted from the responses: the
    // R09 record quotes them, and a run that changes them has to be visible as a change rather than
    // a number in a document that quietly stopped being true.
    let counts = json!({
        "instances": list.len(),
        "actions": projection.actions.len(),
        "responses": responses.len(),
    });

    Ok(json!({
        "case": name,
        // Material that produced no candidate keeps its stated reason, so an unattributable record
        // is visibly unattributed rather than silently absent.
        "problems": problems,
        "lifecycle": lifecycle_filter,
        "counts": counts,
        "responses": responses,
    }))
}

fn main() -> anyhow::Result<()> {
    let cases: PathBuf = std::env::args().nth(1).unwrap_or_else(|| "fixtures/contract".into()).into();
    let output: PathBuf = std::env::args().nth(2).unwrap_or_else(|| "model/drill-output.json".into()).into();
    let mut directories: Vec<PathBuf> = std::fs::read_dir(&cases)?.filter_map(|entry| entry.ok().map(|entry| entry.path())).filter(|path| path.is_dir()).collect();
    directories.sort();
    let results: Vec<Value> = directories.iter().map(|directory| project(directory)).collect::<anyhow::Result<_>>()?;
    let document = json!({
        "generated_by": "crates/contracts/examples/drill.rs",
        "note": "最小贯通演练。投影是内存中的替身，未经过存储层；S05–S08 会替换它。",
        "catalog_version": model::catalog_version(),
        "rule_version": model::catalog_fingerprint(),
        "cases": results,
    });
    std::fs::write(&output, serde_json::to_string_pretty(&document)? + "\n")?;
    println!("drill: {} cases, written to {}", directories.len(), output.display());
    for case in document["cases"].as_array().unwrap() {
        println!(
            "  {}: {} instance(s), {} action(s), {} response(s)",
            case["case"].as_str().unwrap_or("?"),
            case["counts"]["instances"],
            case["counts"]["actions"],
            case["counts"]["responses"]
        );
    }
    Ok(())
}
