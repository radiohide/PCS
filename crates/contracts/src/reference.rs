//! Reference adapters and fixture loading used by the contract checks (R02, R06).
//!
//! These are *reference* implementations for the shared sample set, not the shipping adapters:
//! per-source adapters are S09–S20 work. They live in the library so the adapter check and the
//! contract-case runner exercise the same code instead of drifting apart.
use crate::adapter::*;
use crate::adapters::installation_scope;
use crate::{Envelope, Message};
use serde_json::Value;
use std::path::Path;

/// Converts one persisted protocol message into the evidence inputs it carries. This is the shape
/// S05 will produce from the store; here it is built straight from fixture lines.
pub fn evidence_from(envelope: &Envelope) -> Vec<EvidenceInput> {
    match &envelope.message {
        Message::Snapshot { source, scope, run_id, observed_at, items, .. } => items
            .iter()
            .map(|item| EvidenceInput {
                source: source.clone(),
                item_id: item.id.clone(),
                structure_version: "v1".into(),
                source_scope: scope.clone(),
                evidence: EvidenceRef::snapshot(source, scope, run_id),
                native_record_id: item.id.clone(),
                source_time: None,
                observed_at: observed_at.clone(),
                ingested_at: observed_at.clone(),
                decoding: Decoding { available: true, ..Default::default() },
                payload: item.data.clone(),
            })
            .collect(),
        Message::EventBatch { source, epoch, events, .. } => events
            .iter()
            .map(|event| EvidenceInput {
                source: source.clone(),
                item_id: event.kind.clone(),
                structure_version: "v1".into(),
                source_scope: epoch.clone(),
                evidence: EvidenceRef::event(source, epoch, &event.id),
                native_record_id: event.id.clone(),
                source_time: Some(event.occurred_at.clone()),
                observed_at: event.occurred_at.clone(),
                ingested_at: event.occurred_at.clone(),
                decoding: Decoding { available: true, ..Default::default() },
                payload: event.data.clone(),
            })
            .collect(),
        Message::Gap { .. } => Vec::new(),
    }
}

pub fn read_fixture(path: &Path) -> anyhow::Result<Vec<EvidenceInput>> {
    let text = std::fs::read_to_string(path)?;
    let mut inputs = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        inputs.extend(evidence_from(&crate::parse(line)?));
    }
    Ok(inputs)
}

fn value_type(value: &Value) -> String {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "list",
        Value::Object(_) => "object",
    }
    .into()
}

// ---------- scheduled task: one input is one whole object ----------

pub struct ScheduledTaskAdapter;

impl ScheduledTaskAdapter {
    pub fn field(payload: &Value, field_id: &str, key: &str, kind: FieldKind) -> FieldCandidate {
        match payload.get(key) {
            Some(value) if !value.is_null() => FieldCandidate {
                field_id: field_id.into(),
                field_kind: kind,
                value: Some(value.clone()),
                value_type: value_type(value),
                status: ValueStatus::Known,
                source_time: None,
                derivation: Derivation::SourceProvided,
                conflicts: Vec::new(),
            },
            // The source says nothing about this field; it is recorded as absent, never defaulted.
            _ => FieldCandidate {
                field_id: field_id.into(),
                field_kind: kind,
                value: None,
                value_type: "unknown".into(),
                status: ValueStatus::NotProvided,
                source_time: None,
                derivation: Derivation::SourceProvided,
                conflicts: Vec::new(),
            },
        }
    }
}

pub const SCHEDULED_TASK_FIELDS: [(&str, &str, FieldKind); 6] = [
    ("path", "path", FieldKind::Identity),
    ("name", "name", FieldKind::Configuration),
    ("enabled", "enabled", FieldKind::Configuration),
    ("principal", "principal", FieldKind::Configuration),
    ("actions", "actions", FieldKind::Configuration),
    ("triggers", "triggers", FieldKind::Configuration),
];

impl SourceAdapter for ScheduledTaskAdapter {
    fn adapter_id(&self) -> &'static str {
        "adapter.startup.scheduled_task"
    }
    fn adapt(&self, input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
        // Both of these sources describe scheduled tasks: the base inventory and the detailed
        // configuration. They must converge on one instance, which is what the shared type and the
        // type-decided scope give them.
        if !matches!(input.source.as_str(), "pcscope.startup.tasks" | "pcscope.startup.task-details") {
            return AdapterResult::not_an_object(format!("{} is not scheduled-task material", input.source));
        }
        crate::adapters::startup::scheduled_task(input, context)
    }
}

// ---------- security audit: one input is one flat event ----------

pub struct SecurityEventAdapter;

/// The result semantic an action carries, read from the catalog rather than restated here.
///
/// This is deliberate: an adapter that hardcodes its own outcome per event drifts from the catalog
/// the moment an action is added or its semantics change. Reading it means the catalog is the only
/// place that decides, and a mismatch is impossible rather than merely unlikely.
pub fn declared_outcome(action_id: &str) -> ActionOutcome {
    match crate::model::action_definition(action_id).and_then(|definition| definition["result"].as_str()) {
        Some("succeeded") => ActionOutcome::Succeeded,
        Some("failed") => ActionOutcome::Failed,
        Some("started") => ActionOutcome::Started,
        Some("ended") => ActionOutcome::Ended,
        Some("blocked") => ActionOutcome::Blocked,
        Some("observed") => ActionOutcome::Observed,
        _ => ActionOutcome::Unknown,
    }
}

/// Event identifier to action. Adapter-local knowledge about one source.
pub fn security_action(event_id: i64) -> Option<(&'static str, &'static str)> {
    Some(match event_id {
        4688 => ("execution.process_started", "runtime.process"),
        4689 => ("execution.process_stopped", "runtime.process"),
        4624 => ("identity.logon", "runtime.logon_session"),
        4634 => ("identity.logoff", "runtime.logon_session"),
        4625 => ("identity.logon_failed", "account.user"),
        4698 => ("startup.task_created", "startup.scheduled_task"),
        4699 => ("startup.task_deleted", "startup.scheduled_task"),
        4702 => ("startup.task_updated", "startup.scheduled_task"),
        // TaskScheduler channel: the run log. 100 is the start, 102 the end. They are two stages,
        // so they must never be deduplicated into one success.
        100 => ("startup.task_started", "startup.scheduled_task"),
        102 => ("startup.task_ended", "startup.scheduled_task"),
        4720 => ("identity.account_created", "account.user"),
        4672 => ("identity.special_privileges", "runtime.logon_session"),
        _ => return None,
    })
}

/// Builds the identity a given action targets, listing every key the event did not supply.
pub fn event_identity(type_id: &str, scope: &str, payload: &Value, evidence: &EvidenceRef) -> IdentityCandidate {
    let string = |key: &str| payload.get(key).and_then(|value| value.as_str()).map(str::to_owned);
    let from = evidence.id.clone();
    let mut fields = vec![IdentityField { key: "scope".into(), value: scope.into(), from: "adapter context".into() }];
    let mut missing = Vec::new();
    let keys: Vec<&str> = crate::model::type_definition(type_id)
        .and_then(|definition| definition["identity"]["keys"].as_array())
        .map(|keys| keys.iter().filter_map(|key| key.as_str()).collect())
        .unwrap_or_default();
    for key in keys {
        if key == "scope" {
            continue;
        }
        let value = match key {
            "pid" => string("new_process_id"),
            "native_id" if type_id=="runtime.logon_session" => crate::adapters::event::logon_event_id(payload,scope.contains("/boot:")),
            "native_id" => string("target_user_sid").or_else(|| string("subject_user_sid")),
            "sid" if type_id=="account.user" => crate::adapters::event::account_event_sid(payload),
            "sid" => string("target_user_sid"),
            "path" => string("task_name"),
            _ => None,
        };
        match value {
            // A value the event did not carry is listed as missing; nothing is invented for it.
            None => missing.push(key.to_string()),
            Some(value) => fields.push(IdentityField { key: key.into(), value, from: from.clone() }),
        }
    }
    IdentityCandidate { type_id: type_id.into(), scope: scope.into(), fields, missing, ambiguous: None }
}

impl SourceAdapter for SecurityEventAdapter {
    fn adapter_id(&self) -> &'static str {
        "adapter.eventlog.security"
    }
    fn adapt(&self, input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
        if !input.source.starts_with("pcscope.eventlog.") {
            return AdapterResult::not_an_object(format!("{} is not event-log material", input.source));
        }
        let Some(event_id) = input.payload.get("event_id").and_then(Value::as_i64) else {
            return AdapterResult::failed("event has no event_id; it cannot be classified", true);
        };
        let Some((action_id, target_type)) = security_action(event_id) else {
            // An event with no registered target produces no candidate. The raw record stays
            // retained and searchable; no action is invented to fill the gap.
            return AdapterResult {
                problems: vec![ParseProblem {
                    severity: ProblemSeverity::Warning,
                    message: format!("event id {event_id} is not mapped to a target type; the raw record is retained without a candidate"),
                    field: Some("event_id".into()),
                }],
                ..AdapterResult::not_an_object(format!("security event {event_id} has no registered target"))
            };
        };
        // Event IDs are local to a provider/channel. Application event 100 is not a task start.
        let task_event = matches!(event_id, 100 | 102);
        let expected_provider = if task_event { "Microsoft-Windows-TaskScheduler" } else { "Microsoft-Windows-Security-Auditing" };
        let expected_channel = if task_event { "Microsoft-Windows-TaskScheduler/Operational" } else { "Security" };
        let provider = input.payload.get("provider").and_then(Value::as_str);
        let channel = input.payload.get("channel").and_then(Value::as_str)
            .or_else(||input.source.strip_prefix("pcscope.eventlog."));
        if provider.is_some_and(|value|!value.eq_ignore_ascii_case(expected_provider))
            || !channel.is_some_and(|value|value.eq_ignore_ascii_case(expected_channel)) {
            return AdapterResult::not_an_object("the event provider/channel does not define this action; the original event is retained");
        }
        // The installation an object belongs to, not the account the collection ran as. The
        // category adapters narrow the same way; a reference adapter that did not would put its
        // objects in a scope of their own. For the contract fixtures this is a no-op, because their
        // scopes carry no collecting account to drop.
        let scope = if target_type=="runtime.logon_session" { crate::adapters::event::logon_scope(context) }
            else { installation_scope(&context.collected_scope) };
        let identity = event_identity(target_type, &scope, &input.payload, &input.evidence);
        let target = if action_id=="identity.special_privileges" { crate::adapters::event::special_logon_target(input,context) }
            else if target_type=="account.user" { crate::adapters::event::account_event_target(input,context) }
            else { ActionTarget::Assigned { identity } };
        let outcome = declared_outcome(action_id);
        let mut result = AdapterResult::adapted();
        result.actions.push(ActionCandidate {
            occurrence_key: None, source_record: None,
            target,
            action_id: action_id.into(),
            phase: Some(match event_id {
                4688 | 4689 => "process lifecycle".into(),
                4624 | 4634 => "session".into(),
                4698 | 4699 | 4702 => "task lifecycle".into(),
                100 | 102 => "task run".into(),
                _ => "recorded".into(),
            }),
            result: outcome,
            precision: TimePrecision::Exact,
            occurred_at: Some(input.source_time.clone().unwrap_or_else(|| input.observed_at.clone())),
            interval_from: None,
            interval_until: None,
            observed_at: input.observed_at.clone(),
            field_id: None,
            before: None,
            after: None,
            evidence: vec![input.evidence.clone()],
            actor: input.payload.get("subject_user_name").and_then(Value::as_str).map(str::to_owned),
            program: input.payload.get("new_process_name_protected").and_then(Value::as_str).map(str::to_owned),
        });
        result
    }
}

// ---------- processes: identity needs the creation time, not just the id ----------

pub struct ProcessAdapter;

pub const PROCESS_FIELDS: [(&str, &str, FieldKind); 6] = [
    ("pid", "pid", FieldKind::Identity),
    ("created_at", "created_at", FieldKind::Identity),
    ("image", "image", FieldKind::Configuration),
    ("parent_pid", "parent_pid", FieldKind::Configuration),
    ("session", "session", FieldKind::Configuration),
    ("command_line", "command_line", FieldKind::Configuration),
];

impl SourceAdapter for ProcessAdapter {
    fn adapter_id(&self) -> &'static str {
        "adapter.execution.processes"
    }
    fn adapt(&self, input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
        if input.source != "pcscope.execution.processes" {
            return AdapterResult::not_an_object(format!("{} is not process material", input.source));
        }
        crate::adapters::runtime::processes(input, context)
    }
}

/// Picks the adapter for a source, or `None` when the source has no reference adapter.
pub fn adapter_for(source: &str) -> Option<Box<dyn SourceAdapter>> {
    if matches!(source, "pcscope.startup.tasks" | "pcscope.startup.task-details") {
        return Some(Box::new(ScheduledTaskAdapter));
    }
    if source == "pcscope.execution.processes" {
        return Some(Box::new(ProcessAdapter));
    }
    // Declared adapters are consulted before the event fallback so that a source which emits a
    // snapshot keeps its object adapter: `pcscope.usn.volumes` is one of them.
    if let Some(adapter) = crate::adapters::adapter_for(source) {
        return Some(adapter);
    }
    if crate::adapters::event::is_event_source(source) {
        // One entry point for every event channel: the action and its target come from the catalog
        // when the collector classified the record, and from the event-id table when it did not.
        return Some(Box::new(crate::adapters::event::EventAdapter));
    }
    // The reference adapters above cover the fixtures the contract checks run on; the declared ones
    // (S09–S20) cover the collectors that actually run, and were consulted first.
    None
}
