//! R02 standard adapter contract.
//!
//! An adapter turns already-persisted source material into *candidates*. It does not allocate final
//! object IDs, does not write to the database, and does not call Windows again to fetch anything
//! that is missing. Everything it produces is a proposal that `validate` accepts or rejects against
//! the model catalog, and that the identity engine (S06) and projection (S07) later act on.
//!
//! Retention rules that shape the design:
//!   * unknown fields stay in `EvidenceInput::payload` — candidates never copy raw material;
//!   * a missing value is carried as an explicit status, never guessed or defaulted;
//!   * the same input always yields the same candidates, so replays are comparable;
//!   * a failure is a stated outcome with a reason and a retryability flag, not an empty result.
use crate::model;
use crate::semantics::{canonical_time, compare_canonical};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

// ---------- input ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    /// A row in `records` — an event or a derived observation.
    Record,
    /// A complete snapshot, addressed by source, scope and run.
    Snapshot,
    /// A deduplicated source event identity.
    Event,
}

/// Stable pointer to persisted material. This is what a candidate cites; it never carries the
/// material itself, so large payloads are not copied per field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceRef {
    pub kind: EvidenceKind,
    pub id: String,
}
impl EvidenceRef {
    pub fn record(seq: i64) -> Self {
        Self { kind: EvidenceKind::Record, id: seq.to_string() }
    }
    pub fn snapshot(source: &str, scope: &str, run_id: &str) -> Self {
        Self { kind: EvidenceKind::Snapshot, id: format!("{source}|{scope}|{run_id}") }
    }
    /// Event dedup identity is (source, epoch, event id); the same triple identifies the evidence.
    pub fn event(source: &str, epoch: &str, event_id: &str) -> Self {
        Self { kind: EvidenceKind::Event, id: format!("{source}|{epoch}|{event_id}") }
    }
}

/// What the reader could and could not decode. Truncation, masking and per-field read failures are
/// recorded here rather than being flattened into "missing".
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Decoding {
    pub available: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub problems: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unreadable_fields: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub masked_fields: Vec<String>,
}

/// One piece of persisted source material, exactly as it was stored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceInput {
    /// Source identifier, e.g. `pcscope.startup.tasks`.
    pub source: String,
    /// Collection item or classification identity within the source.
    pub item_id: String,
    /// Version of the *source structure*, not of the model catalog.
    pub structure_version: String,
    /// Scope as delivered by the collector. Business scope is decided by the type, not by this.
    pub source_scope: String,
    pub evidence: EvidenceRef,
    /// Identity the source itself provides, empty when it provides none.
    pub native_record_id: String,
    pub source_time: Option<String>,
    pub observed_at: String,
    pub ingested_at: String,
    pub decoding: Decoding,
    /// The material verbatim. Unknown fields remain here and are never rewritten by an adapter.
    pub payload: Value,
}

/// Context the adapter may rely on. Deliberately does not include a final object identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdapterContext {
    pub catalog_version: String,
    pub rule_version: String,
    pub machine_id: String,
    pub installation_id: String,
    /// Present only when the source states it; never inferred to fill a gap.
    pub boot_id: Option<String>,
    /// The scope the collection actually covered, which may be narrower than the type's scope.
    pub collected_scope: String,
    /// Whether the source signalled a complete observation.
    pub complete: bool,
}

// ---------- candidates ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    Identity,
    Configuration,
    State,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueStatus {
    Known,
    NotProvided,
    Unreadable,
    NotApplicable,
    Conflicting,
    Masked,
    EvidencePruned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Derivation {
    SourceProvided,
    Decoded,
    SnapshotComparison,
    Rule,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConflictValue {
    pub value: Value,
    pub source: String,
    pub source_time: Option<String>,
}

/// One identity component with the place it came from, so the identity can be explained later.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IdentityField {
    pub key: String,
    pub value: String,
    pub from: String,
}

/// A proposed identity. Not a final object ID: the same candidate from two sources must be
/// reconcilable by the identity engine, so nothing here is derived from a collection shard,
/// request identifier or plugin name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IdentityCandidate {
    pub type_id: String,
    /// Business scope decided by the type, e.g. install scope or user SID.
    pub scope: String,
    pub fields: Vec<IdentityField>,
    /// Keys the type's strategy requires but this input did not provide. Non-empty means the
    /// identity is provisional; it must never be replaced by an invented value.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambiguous: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldCandidate {
    pub field_id: String,
    pub field_kind: FieldKind,
    /// Present exactly when status is `known`, `masked` or `conflicting`; absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    pub value_type: String,
    pub status: ValueStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_time: Option<String>,
    pub derivation: Derivation,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflicts: Vec<ConflictValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObservationCandidate {
    pub identity: IdentityCandidate,
    pub fields: Vec<FieldCandidate>,
    pub observed_at: String,
    /// At least one reference is required: an observation nothing supports is not admissible.
    pub evidence: Vec<EvidenceRef>,
    pub complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimePrecision {
    Exact,
    Interval,
    ObservationOnly,
}

/// Outcome semantics, mirroring the catalog's `result_semantics`. `started` never means succeeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionOutcome {
    Succeeded,
    Failed,
    Started,
    Ended,
    Blocked,
    Observed,
    Unknown,
}

/// Either a proposed target identity or a stated reason why none could be determined. An action is
/// never silently attached to a guessed object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "target", rename_all = "snake_case")]
pub enum ActionTarget {
    Assigned { identity: IdentityCandidate },
    Unassigned { reason: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionCandidate {
    /// Stable source occurrence identity, when the source provides one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occurrence_key: Option<String>,
    /// Source-recorded operation data, distinct from before/after field changes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_record: Option<Value>,
    pub target: ActionTarget,
    pub action_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    pub result: ActionOutcome,
    pub precision: TimePrecision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occurred_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval_until: Option<String>,
    pub observed_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<Value>,
    pub evidence: Vec<EvidenceRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program: Option<String>,
}

// ---------- result ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProblemSeverity {
    /// Recorded, the rest of the result is still usable.
    Note,
    /// A field or item was skipped; the result is incomplete.
    Warning,
    /// The input could not be interpreted.
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParseProblem {
    pub severity: ProblemSeverity,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

/// What an adapter did with one input. There is no "empty means failure": a source that is not an
/// object, or that failed, says so explicitly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum AdapterOutcome {
    Adapted,
    /// The material is legitimate but describes no object (a sampling value, a source status).
    NotAnObject { reason: String },
    Failed { reason: String, retryable: bool },
    /// Deliberately not handled, with a reason that stays visible.
    Skipped { reason: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdapterResult {
    pub outcome: AdapterOutcome,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub observations: Vec<ObservationCandidate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<ActionCandidate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub problems: Vec<ParseProblem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}
impl AdapterResult {
    pub fn adapted() -> Self {
        Self { outcome: AdapterOutcome::Adapted, observations: Vec::new(), actions: Vec::new(), problems: Vec::new(), notes: Vec::new() }
    }
    pub fn not_an_object(reason: impl Into<String>) -> Self {
        Self { outcome: AdapterOutcome::NotAnObject { reason: reason.into() }, observations: Vec::new(), actions: Vec::new(), problems: Vec::new(), notes: Vec::new() }
    }
    pub fn failed(reason: impl Into<String>, retryable: bool) -> Self {
        Self { outcome: AdapterOutcome::Failed { reason: reason.into(), retryable }, observations: Vec::new(), actions: Vec::new(), problems: Vec::new(), notes: Vec::new() }
    }
    /// A result is complete only when it was adapted and nothing above `note` was reported. A
    /// failed, non-object or skipped result is never complete, even when it carries no problems.
    pub fn complete(&self) -> bool {
        matches!(self.outcome, AdapterOutcome::Adapted)
            && !self.problems.iter().any(|p| p.severity != ProblemSeverity::Note)
    }
}

/// The single interface every source adapter implements.
pub trait SourceAdapter {
    /// Stable adapter identity, used in diagnostics and by the mapping registry (R07).
    fn adapter_id(&self) -> &'static str;
    fn adapt(&self, input: &EvidenceInput, context: &AdapterContext) -> AdapterResult;
}

// ---------- validation ----------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContractViolation {
    /// Where in the result the violation is, e.g. `observations[0].fields[2].value`.
    pub at: String,
    pub message: String,
}

fn conflict_ok(status: ValueStatus) -> bool {
    matches!(status, ValueStatus::Known | ValueStatus::Masked | ValueStatus::Conflicting)
}

/// Checks a result against the model catalog and the contract rules. A violation means the
/// candidate set must not be projected: the adapter is wrong, not the catalog.
pub fn validate(result: &AdapterResult, context: &AdapterContext) -> Vec<ContractViolation> {
    let mut found = Vec::new();
    let mut fail = |at: String, message: String| found.push(ContractViolation { at, message });

    if context.rule_version.is_empty() {
        fail("context.rule_version".into(), "rule version must be stated".into());
    }
    if context.catalog_version != model::catalog_version() {
        fail("context.catalog_version".into(), format!("catalog version {} does not match the embedded catalog {}", context.catalog_version, model::catalog_version()));
    }

    let check_identity = |identity: &IdentityCandidate, at: &str, fail: &mut dyn FnMut(String, String)| {
        let Some(definition) = model::type_definition(&identity.type_id) else {
            fail(format!("{at}.type_id"), format!("type \"{}\" is not registered in the catalog", identity.type_id));
            return;
        };
        if identity.scope.is_empty() {
            fail(format!("{at}.scope"), "identity scope must be stated".into());
        }
        let declared: Vec<&str> = definition["identity"]["keys"].as_array()
            .map(|keys| keys.iter().filter_map(|k| k.as_str()).collect()).unwrap_or_default();
        for field in &identity.fields {
            if field.value.is_empty() {
                fail(format!("{at}.fields[{}]", field.key), "identity value must not be empty; use `missing` instead".into());
            }
            if field.from.is_empty() {
                fail(format!("{at}.fields[{}]", field.key), "identity value must state where it came from".into());
            }
        }
        let provided: Vec<&str> = identity.fields.iter().map(|f| f.key.as_str()).collect();
        for required in &declared {
            // A composite strategy declares its keys per type; those arrive as field entries.
            if required.starts_with('@') { continue; }
            if !provided.contains(required) && !identity.missing.contains(&required.to_string()) {
                fail(format!("{at}.missing"), format!("required identity key \"{required}\" is neither provided nor listed as missing"));
            }
        }
    };

    for (index, observation) in result.observations.iter().enumerate() {
        let at = format!("observations[{index}]");
        check_identity(&observation.identity, &format!("{at}.identity"), &mut fail);
        if observation.evidence.is_empty() {
            fail(format!("{at}.evidence"), "an observation must cite at least one piece of evidence".into());
        }
        for (position, field) in observation.fields.iter().enumerate() {
            let field_at = format!("{at}.fields[{position}]");
            if !model::has_field(&observation.identity.type_id, &field.field_id) {
                fail(format!("{field_at}.field_id"), format!("field \"{}\" is not declared by type \"{}\"", field.field_id, observation.identity.type_id));
            }
            match field.status {
                ValueStatus::Known => {
                    if field.value.is_none() {
                        fail(format!("{field_at}.value"), "a known field must carry its value".into());
                    }
                }
                ValueStatus::Masked | ValueStatus::Conflicting => {
                    if field.value.is_none() {
                        fail(format!("{field_at}.value"), "a masked or conflicting field must keep its value".into());
                    }
                }
                _ => {
                    if field.value.is_some() {
                        fail(format!("{field_at}.value"), format!("status {:?} must not carry a value: a missing value is never filled with a guess", field.status));
                    }
                }
            }
            if field.status == ValueStatus::Conflicting && field.conflicts.len() < 2 {
                fail(format!("{field_at}.conflicts"), "a conflicting field must keep its competing values".into());
            }
            if field.value.is_some() && !conflict_ok(field.status) {
                fail(format!("{field_at}.status"), "a carried value needs a status that admits one".into());
            }
        }
    }

    for (index, action) in result.actions.iter().enumerate() {
        let at = format!("actions[{index}]");
        match model::action_definition(&action.action_id) {
            None => fail(format!("{at}.action_id"), format!("action \"{}\" is not registered in the catalog", action.action_id)),
            Some(definition) => {
                if let ActionTarget::Assigned { identity } = &action.target {
                    let expected = definition["target_type"].as_str();
                    if let Some(expected) = expected {
                        if expected != identity.type_id {
                            fail(format!("{at}.target"), format!("action \"{}\" targets \"{expected}\" but the candidate names \"{}\"", action.action_id, identity.type_id));
                        }
                    }
                }
            }
        }
        match &action.target {
            ActionTarget::Assigned { identity } => check_identity(identity, &format!("{at}.target.identity"), &mut fail),
            ActionTarget::Unassigned { reason } => {
                if reason.trim().is_empty() {
                    fail(format!("{at}.target"), "an unassigned action must state why it could not be assigned".into());
                }
            }
        }
        if action.evidence.is_empty() {
            fail(format!("{at}.evidence"), "an action must cite at least one piece of evidence".into());
        }
        match action.precision {
            TimePrecision::Exact => {
                if action.occurred_at.is_none() {
                    fail(format!("{at}.occurred_at"), "an exact action needs its occurrence time".into());
                }
            }
            TimePrecision::Interval => {
                if action.interval_from.is_none() || action.interval_until.is_none() {
                    fail(format!("{at}.interval"), "an interval action needs both bounds".into());
                }
                if action.occurred_at.is_some() {
                    fail(format!("{at}.occurred_at"), "an interval action must not claim an exact time".into());
                }
                // An interval has to have room for a change. Two observations that take effect at the
                // same time are §3's agreement-or-conflict, never a span, and a span from an instant
                // to itself claims the change happened between a time and itself — the one change no
                // source observed. The bounds are canonicalised before comparing: raw text would let
                // `…09:00:00Z` and `…09:00:00.000Z` pass as two different instants.
                if let (Some(from), Some(until)) = (&action.interval_from, &action.interval_until) {
                    match (canonical_time(from), canonical_time(until)) {
                        (Ok(from), Ok(until)) => {
                            if compare_canonical(&from, &until) != std::cmp::Ordering::Less {
                                fail(format!("{at}.interval"), format!("an interval from {from} to {until} has no room for a change; a change takes effect later than it began"));
                            }
                        }
                        _ => fail(format!("{at}.interval"), "interval bounds must be UTC RFC 3339 timestamps; a bound that is not a time cannot bound anything".into()),
                    }
                }
            }
            TimePrecision::ObservationOnly => {
                if action.occurred_at.is_some() || action.interval_from.is_some() {
                    fail(format!("{at}.precision"), "an observation-only action must not claim a time it cannot know".into());
                }
            }
        }
        if let Some(field_id) = &action.field_id {
            if let ActionTarget::Assigned { identity } = &action.target {
                if !model::has_field(&identity.type_id, field_id) {
                    fail(format!("{at}.field_id"), format!("field \"{field_id}\" is not declared by type \"{}\"", identity.type_id));
                }
            }
        }
        if action.before.is_some() || action.after.is_some() {
            if action.field_id.is_none() {
                fail(format!("{at}.field_id"), "a before/after pair must name the field it changed".into());
            }
            // A change has two sides, and a missing value is not a value. A pair with one side absent
            // is not a change: it reports a value that appeared or vanished. That reading is what
            // produced phantom actions for a task whose two sources describe it through different
            // fields — each source said nothing about the fields the other carried, and its silence
            // was compared as if it were a value.
            // SCM explicitly records the resulting state, but not the preceding one. This is a
            // qualified source statement, not a snapshot comparison against a missing field.
            let service_state_statement = action.action_id == "startup.service_state_changed"
                && action.field_id.as_deref() == Some("state") && action.before.is_none()
                && matches!(action.after.as_ref().and_then(Value::as_str), Some("Running" | "Stopped"))
                && action.precision == TimePrecision::Exact && action.result == ActionOutcome::Observed;
            if (action.before.is_none() || action.after.is_none()) && !service_state_statement {
                fail(format!("{at}.before_after"), "a change action must state both before and after; a missing value is not a value and cannot be one side of a change".into());
            }
        }
    }

    match &result.outcome {
        AdapterOutcome::Failed { reason, .. } => {
            if reason.trim().is_empty() {
                fail("outcome".into(), "a failed adapter must state a reason".into());
            }
        }
        AdapterOutcome::NotAnObject { reason } | AdapterOutcome::Skipped { reason } => {
            if reason.trim().is_empty() {
                fail("outcome".into(), "a non-object or skipped outcome must state a reason".into());
            }
            if !result.observations.is_empty() || !result.actions.is_empty() {
                fail("outcome".into(), "a non-object outcome must not also carry candidates".into());
            }
        }
        AdapterOutcome::Adapted => {}
    }
    if matches!(result.outcome, AdapterOutcome::Adapted)
        && result.observations.is_empty()
        && result.actions.is_empty()
        && !result.problems.iter().any(|p| p.severity != ProblemSeverity::Note)
    {
        fail("outcome".into(), "an adapted result with no candidates and no problem is not admissible".into());
    }
    found
}

/// The contract's serialised shape. Kept alongside the Rust types so a non-Rust consumer (the UI,
/// or a mapping report) can be checked against the same structure.
pub fn contract_schema() -> Value {
    json!({
        "contract_version": 1,
        "catalog_version": model::catalog_version(),
        "rule_version": model::catalog_fingerprint(),
        "evidence_kind": ["record", "snapshot", "event"],
        "field_kind": ["identity", "configuration", "state"],
        "value_status": ["known", "not_provided", "unreadable", "not_applicable", "conflicting", "masked", "evidence_pruned"],
        "derivation": ["source_provided", "decoded", "snapshot_comparison", "rule"],
        "time_precision": ["exact", "interval", "observation_only"],
        "action_outcome": ["succeeded", "failed", "started", "ended", "blocked", "observed", "unknown"],
        "problem_severity": ["note", "warning", "error"],
        "adapter_outcome": ["adapted", "not_an_object", "failed", "skipped"],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> AdapterContext {
        AdapterContext {
            catalog_version: model::catalog_version().into(),
            rule_version: model::catalog_fingerprint(),
            machine_id: "m".into(),
            installation_id: "i".into(),
            boot_id: None,
            collected_scope: "machine:m".into(),
            complete: true,
        }
    }
    fn identity(type_id: &str) -> IdentityCandidate {
        let keys: Vec<IdentityField> = model::type_definition(type_id)
            .and_then(|d| d["identity"]["keys"].as_array())
            .map(|keys| keys.iter().filter_map(|k| k.as_str()).collect::<Vec<&str>>())
            .unwrap_or_default()
            .into_iter()
            .map(|key| IdentityField { key: key.into(), value: "v".into(), from: "test".into() })
            .collect();
        IdentityCandidate { type_id: type_id.into(), scope: "scope".into(), fields: keys, missing: Vec::new(), ambiguous: None }
    }
    fn field(field_id: &str, status: ValueStatus, value: Option<Value>) -> FieldCandidate {
        FieldCandidate { field_id: field_id.into(), field_kind: FieldKind::Configuration, value, value_type: "string".into(), status, source_time: None, derivation: Derivation::SourceProvided, conflicts: Vec::new() }
    }
    /// A valid field-change action, so a test can vary the one thing it is about.
    fn result_action() -> ActionCandidate {
        ActionCandidate {
            occurrence_key: None, source_record: None,
            target: ActionTarget::Assigned { identity: identity("startup.scheduled_task") },
            action_id: "observed.configuration_changed".into(),
            phase: None,
            result: ActionOutcome::Observed,
            precision: TimePrecision::Interval,
            occurred_at: None,
            interval_from: Some("2026-09-11T00:00:00Z".into()),
            interval_until: Some("2026-09-11T01:00:00Z".into()),
            observed_at: "2026-09-11T01:00:00Z".into(),
            field_id: Some("enabled".into()),
            before: Some(json!(true)),
            after: Some(json!(false)),
            evidence: vec![EvidenceRef::record(1)],
            actor: None,
            program: None,
        }
    }

    #[test]
    fn an_unregistered_type_is_rejected() {
        let mut result = AdapterResult::adapted();
        result.observations.push(ObservationCandidate {
            identity: identity("startup.no_such_type"),
            fields: vec![],
            observed_at: "2026-09-11T00:00:00Z".into(),
            evidence: vec![EvidenceRef::record(1)],
            complete: true,
        });
        let violations = validate(&result, &context());
        assert!(violations.iter().any(|v| v.message.contains("not registered")), "{violations:?}");
    }

    #[test]
    fn an_unregistered_field_is_rejected() {
        let mut result = AdapterResult::adapted();
        result.observations.push(ObservationCandidate {
            identity: identity("startup.scheduled_task"),
            fields: vec![field("not_a_field", ValueStatus::Known, Some(json!("x")))],
            observed_at: "2026-09-11T00:00:00Z".into(),
            evidence: vec![EvidenceRef::record(1)],
            complete: true,
        });
        let violations = validate(&result, &context());
        assert!(violations.iter().any(|v| v.message.contains("not declared by type")), "{violations:?}");
    }

    #[test]
    fn a_missing_value_is_never_filled_in() {
        let mut result = AdapterResult::adapted();
        result.observations.push(ObservationCandidate {
            identity: identity("startup.scheduled_task"),
            // A value carried alongside a non-knowing status is exactly the guess the contract forbids.
            fields: vec![field("enabled", ValueStatus::NotProvided, Some(json!(true)))],
            observed_at: "2026-09-11T00:00:00Z".into(),
            evidence: vec![EvidenceRef::record(1)],
            complete: true,
        });
        let violations = validate(&result, &context());
        assert!(violations.iter().any(|v| v.message.contains("never filled with a guess")), "{violations:?}");

        let mut missing_value = AdapterResult::adapted();
        missing_value.observations.push(ObservationCandidate {
            identity: identity("startup.scheduled_task"),
            fields: vec![field("enabled", ValueStatus::Known, None)],
            observed_at: "2026-09-11T00:00:00Z".into(),
            evidence: vec![EvidenceRef::record(1)],
            complete: true,
        });
        assert!(validate(&missing_value, &context()).iter().any(|v| v.message.contains("must carry its value")));
    }

    #[test]
    fn an_interval_action_must_not_claim_an_exact_time() {
        let mut result = AdapterResult::adapted();
        result.actions.push(ActionCandidate {
            occurrence_key: None, source_record: None,
            target: ActionTarget::Assigned { identity: identity("startup.scheduled_task") },
            action_id: "observed.configuration_changed".into(),
            phase: None,
            result: ActionOutcome::Observed,
            precision: TimePrecision::Interval,
            occurred_at: Some("2026-09-11T00:00:00Z".into()),
            interval_from: Some("2026-09-11T00:00:00Z".into()),
            interval_until: Some("2026-09-11T01:00:00Z".into()),
            observed_at: "2026-09-11T01:00:00Z".into(),
            field_id: None,
            before: None,
            after: None,
            evidence: vec![EvidenceRef::record(1)],
            actor: None,
            program: None,
        });
        let violations = validate(&result, &context());
        assert!(violations.iter().any(|v| v.message.contains("must not claim an exact time")), "{violations:?}");
    }

    #[test]
    fn an_interval_must_have_room_for_a_change() {
        let mut result = AdapterResult::adapted();
        result.actions.push(ActionCandidate {
            occurrence_key: None, source_record: None,
            target: ActionTarget::Assigned { identity: identity("startup.scheduled_task") },
            action_id: "observed.configuration_changed".into(),
            phase: None,
            result: ActionOutcome::Observed,
            precision: TimePrecision::Interval,
            occurred_at: None,
            // Two sources describing the same field at the same time agree or conflict; neither is a
            // change over time, so a span from an instant to itself cannot be one.
            interval_from: Some("2026-09-11T00:00:00Z".into()),
            interval_until: Some("2026-09-11T00:00:00Z".into()),
            observed_at: "2026-09-11T00:00:00Z".into(),
            field_id: Some("enabled".into()),
            before: Some(json!(true)),
            after: Some(json!(false)),
            evidence: vec![EvidenceRef::record(1)],
            actor: None,
            program: None,
        });
        let violations = validate(&result, &context());
        assert!(violations.iter().any(|v| v.message.contains("no room for a change")), "{violations:?}");
    }

    #[test]
    fn interval_bounds_are_compared_as_times_not_as_text() {
        let mut bare = AdapterResult::adapted();
        let mut action = result_action();
        action.interval_from = Some("2026-09-11T00:00:00Z".into());
        action.interval_until = Some("2026-09-11T00:00:00.000Z".into());
        bare.actions.push(action);
        let violations = validate(&bare, &context());
        // The same instant written two ways is still the same instant.
        assert!(violations.iter().any(|v| v.message.contains("no room for a change")), "{violations:?}");

        let mut unparseable = AdapterResult::adapted();
        let mut action = result_action();
        action.interval_from = Some("2026-09-11T00:00:00Z".into());
        action.interval_until = Some("2026-09-11 00:00:01".into());
        unparseable.actions.push(action);
        let violations = validate(&unparseable, &context());
        assert!(violations.iter().any(|v| v.message.contains("cannot bound anything")), "{violations:?}");
    }

    #[test]
    fn a_change_must_state_both_sides() {
        let mut result = AdapterResult::adapted();
        let mut action = result_action();
        // The field the other source never mentioned is not a value, so this is not a change.
        action.before = Some(json!(true));
        action.after = None;
        result.actions.push(action);
        let violations = validate(&result, &context());
        assert!(violations.iter().any(|v| v.message.contains("both before and after")), "{violations:?}");
    }

    #[test]
    fn an_action_on_the_wrong_type_is_rejected() {
        let mut result = AdapterResult::adapted();
        result.actions.push(ActionCandidate {
            // startup.task_created targets startup.scheduled_task.
            occurrence_key: None, source_record: None,
            target: ActionTarget::Assigned { identity: identity("account.user") },
            action_id: "startup.task_created".into(),
            phase: None,
            result: ActionOutcome::Succeeded,
            precision: TimePrecision::Exact,
            occurred_at: Some("2026-09-11T00:00:00Z".into()),
            interval_from: None,
            interval_until: None,
            observed_at: "2026-09-11T00:00:00Z".into(),
            field_id: None,
            before: None,
            after: None,
            evidence: vec![EvidenceRef::record(1)],
            actor: None,
            program: None,
        });
        let violations = validate(&result, &context());
        assert!(violations.iter().any(|v| v.message.contains("but the candidate names")), "{violations:?}");
    }

    #[test]
    fn an_evidence_free_or_unexplained_result_is_rejected() {
        let mut result = AdapterResult::adapted();
        result.observations.push(ObservationCandidate {
            identity: identity("startup.scheduled_task"),
            fields: vec![],
            observed_at: "2026-09-11T00:00:00Z".into(),
            evidence: vec![],
            complete: true,
        });
        assert!(validate(&result, &context()).iter().any(|v| v.message.contains("cite at least one piece")));

        let empty = AdapterResult::adapted();
        assert!(validate(&empty, &context()).iter().any(|v| v.message.contains("not admissible")));

        let unassigned = AdapterResult { actions: vec![ActionCandidate {
            occurrence_key: None, source_record: None,
            target: ActionTarget::Unassigned { reason: "  ".into() },
            action_id: "files.usn_changed".into(), phase: None, result: ActionOutcome::Observed,
            precision: TimePrecision::ObservationOnly, occurred_at: None, interval_from: None, interval_until: None,
            observed_at: "2026-09-11T00:00:00Z".into(), field_id: None, before: None, after: None,
            evidence: vec![EvidenceRef::record(1)], actor: None, program: None,
        }], ..AdapterResult::adapted() };
        assert!(validate(&unassigned, &context()).iter().any(|v| v.message.contains("must state why")));
    }

    #[test]
    fn a_catalog_mismatch_is_rejected() {
        let mut stale = context();
        stale.catalog_version = "0.0.1".into();
        let mut result = AdapterResult::adapted();
        result.observations.push(ObservationCandidate {
            identity: identity("startup.scheduled_task"),
            fields: vec![],
            observed_at: "2026-09-11T00:00:00Z".into(),
            evidence: vec![EvidenceRef::record(1)],
            complete: true,
        });
        assert!(validate(&result, &stale).iter().any(|v| v.message.contains("does not match the embedded catalog")));
    }

    #[test]
    fn an_invalid_enum_value_is_rejected_rather_than_defaulted() {
        // A typo or an invented status must fail loudly. If an unknown value deserialised to some
        // default, a wrong meaning would flow silently into the projection.
        for (label, json) in [
            ("value status", r#"{"field_id":"enabled","field_kind":"configuration","value_type":"boolean","status":"probably","derivation":"source_provided"}"#),
            ("field kind", r#"{"field_id":"enabled","field_kind":"guess","value_type":"boolean","status":"known","derivation":"source_provided"}"#),
            ("derivation", r#"{"field_id":"enabled","field_kind":"configuration","value_type":"boolean","status":"known","derivation":"invented"}"#),
        ] {
            let parsed = serde_json::from_str::<FieldCandidate>(json);
            assert!(parsed.is_err(), "an invalid {label} must be rejected: {json}");
        }
        // Time precision and outcome carry the same guarantee.
        assert!(serde_json::from_str::<TimePrecision>(r#""roughly""#).is_err());
        assert!(serde_json::from_str::<ActionOutcome>(r#""maybe""#).is_err());
        assert!(serde_json::from_str::<EvidenceKind>(r#""guess""#).is_err());
        // And an unknown action target shape is rejected too.
        assert!(serde_json::from_str::<ActionTarget>(r#"{"target":"probably","identity":{}}"#).is_err());
    }

    #[test]
    fn only_an_adapted_result_without_problems_is_complete() {
        let adapted = AdapterResult::adapted();
        assert!(adapted.complete());
        // A failure with no problems recorded is still not a complete result. Reading `complete`
        // as "no problems" would have called these three successful.
        assert!(!AdapterResult::failed("boom", true).complete());
        assert!(!AdapterResult::not_an_object("a sampling value").complete());
        assert!(!AdapterResult { outcome: AdapterOutcome::Skipped { reason: "not handled".into() }, ..AdapterResult::adapted() }.complete());

        let mut warned = AdapterResult::adapted();
        warned.problems.push(ParseProblem { severity: ProblemSeverity::Warning, message: "a field was skipped".into(), field: None });
        assert!(!warned.complete());
        let mut noted = AdapterResult::adapted();
        noted.problems.push(ParseProblem { severity: ProblemSeverity::Note, message: "informational".into(), field: None });
        assert!(noted.complete(), "a note does not make a result incomplete");
    }

    #[test]
    fn the_schema_and_round_trip_are_stable() {
        let schema = contract_schema();
        assert_eq!(schema["contract_version"], 1);
        assert_eq!(schema["catalog_version"], model::catalog_version());
        assert_eq!(schema["value_status"].as_array().unwrap().len(), 7);

        // Serialisation is part of the contract: the same result must survive a round trip.
        let mut result = AdapterResult::adapted();
        result.observations.push(ObservationCandidate {
            identity: identity("startup.scheduled_task"),
            fields: vec![field("enabled", ValueStatus::Known, Some(json!(true)))],
            observed_at: "2026-09-11T00:00:00Z".into(),
            evidence: vec![EvidenceRef::record(7)],
            complete: true,
        });
        let text = serde_json::to_string(&result).unwrap();
        let back: AdapterResult = serde_json::from_str(&text).unwrap();
        assert_eq!(result, back);
        assert_eq!(serde_json::to_string(&back).unwrap(), text, "serialisation must be deterministic");
    }
}
