//! S09–S20 per-source adapters.
//!
//! One module per object category, each declaring how its sources' material becomes candidates.
//! The declaration is checked against the collector's real output, not invented: a source emits
//! `Item(id, data)` where `data` is the named-property table the collector built, and the specs
//! below name exactly those keys.
//!
//! Three shapes occur, and each has one helper here:
//!
//!   * **one payload is one object** — the common case: identity from the item, fields from the
//!     data table (`object_candidate`);
//!   * **one payload expands into several objects** — a group's member list is a candidate per
//!     member, an account's rights list is a candidate per privilege (`expand` in the spec);
//!   * **one source describes one object through several item streams** — the machine identity
//!     arrives as one item from `Win32_ComputerSystem` and another from its product record, and
//!     both belong to the machine the collection scope already names.
//!
//! What no spec may do: fill in a field the source did not provide. A key the collector did not
//! send becomes `NotProvided`, and a value it explicitly sent as null stays absent — the sources
//! that cannot report a creation time say so, and that statement is the answer.
use crate::adapter::*;
use serde_json::Value;

pub mod account;
pub mod software;
pub mod runtime;
pub mod startup;
pub mod policy;
pub mod shell;
pub mod hardware;
pub mod network;
pub mod virtualization;
pub mod event;
pub mod desktop;
pub mod security;
pub mod file;
pub mod source_gaps;
pub mod retained_history;

/// How a source's material becomes candidates. `build` is the source's whole behaviour, so a spec
/// can be a flat mapping or something with structure without the registry caring which.
pub struct SourceSpec {
    pub source: &'static str,
    /// The type this source produces, when it produces exactly one. Sources that decide the target
    /// per item (an event log above all) state it themselves and leave this `None`.
    pub type_id: Option<&'static str>,
    pub build: fn(&EvidenceInput, &AdapterContext) -> AdapterResult,
}

pub struct DeclaredAdapter {
    pub spec: &'static SourceSpec,
}

impl SourceAdapter for DeclaredAdapter {
    fn adapter_id(&self) -> &'static str {
        self.spec.source
    }
    fn adapt(&self, input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
        (self.spec.build)(input, context)
    }
}

/// The adapter for a source, when one has been declared.
pub fn adapter_for(source: &str) -> Option<Box<dyn SourceAdapter>> {
    spec_for(source).map(|spec| Box::new(DeclaredAdapter { spec }) as Box<dyn SourceAdapter>)
}

pub fn spec_for(source: &str) -> Option<&'static SourceSpec> {
    CATEGORIES.iter().flat_map(|category| category.iter()).find(|spec| spec.source == source)
}

/// Every declared spec, for the coverage reconciliation S21 performs.
pub fn all_specs() -> impl Iterator<Item = &'static SourceSpec> {
    CATEGORIES.iter().flat_map(|category| category.iter())
}

const CATEGORIES: &[&[SourceSpec]] = &[account::SOURCES, software::SOURCES, runtime::SOURCES, startup::SOURCES, policy::SOURCES, shell::SOURCES, hardware::SOURCES, network::SOURCES, virtualization::SOURCES, desktop::SOURCES, security::SOURCES, file::SOURCES, source_gaps::SOURCES];

// ---------- shared shapes ----------

/// Reads a named property out of the collector's data table.
pub fn text(payload: &Value, key: &str) -> Option<String> {
    payload.get(key).and_then(|value| match value {
        Value::String(text) => Some(text.clone()),
        // A number or a boolean the collector wrote where a string was expected is still what the
        // source said. Turning it into a string here is a formatting choice, not a guess.
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    })
}

pub fn value_type(value: &Value) -> String {
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

/// Maps one declared field from the data table. A key the source did not send, or sent as null, is
/// recorded as absent; it is never defaulted to an empty string or a zero.
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

/// Reads a registry value out of either registry-shaped item.
///
/// The PowerShell collectors put the values under `values`, keyed by name, each with a kind and the
/// value. The software plugin writes the same map and also keeps the display fields at the top level
/// for readers of the older shape. Both are the same fact; this reads whichever is there.
pub fn registry_value(payload: &Value, name: &str) -> Option<Value> {
    let nested = payload
        .get("values")
        .and_then(|values| values.get(name))
        .and_then(|entry| entry.get("value"))
        .filter(|value| !value.is_null())
        .cloned();
    nested.or_else(|| payload.get(name).filter(|value| !value.is_null()).cloned())
}

/// Maps one registry-backed field. A value the source masked stays masked rather than becoming a
/// value that participates in comparison.
pub fn registry_field(payload: &Value, field_id: &str, name: &str, kind: FieldKind) -> FieldCandidate {
    match registry_value(payload, name) {
        Some(value) if crate::semantics::is_mask_sentinel(&value) => FieldCandidate {
            field_id: field_id.into(),
            field_kind: kind,
            value: Some(value),
            value_type: "string".into(),
            status: ValueStatus::Masked,
            source_time: None,
            derivation: Derivation::SourceProvided,
            conflicts: Vec::new(),
        },
        Some(value) => FieldCandidate {
            field_id: field_id.into(),
            field_kind: kind,
            value: Some(value.clone()),
            value_type: value_type(&value),
            status: ValueStatus::Known,
            source_time: None,
            derivation: Derivation::SourceProvided,
            conflicts: Vec::new(),
        },
        None => field(payload, field_id, name, kind),
    }
}

/// A field the source states it could not read, as distinct from one it did not mention.
///
/// The difference is the whole point: "this group's members could not be read" and "this group has
/// no members" are different facts, and only one of them is true when access is denied.
pub fn unreadable(field_id: &str, kind: FieldKind) -> FieldCandidate {
    FieldCandidate {
        field_id: field_id.into(),
        field_kind: kind,
        value: None,
        value_type: "unknown".into(),
        status: ValueStatus::Unreadable,
        source_time: None,
        derivation: Derivation::SourceProvided,
        conflicts: Vec::new(),
    }
}

/// The installation an object belongs to, out of the scope a collection covered.
///
/// A collection runs as some account, and that account appears in the collected scope as a
/// `user:` segment. For an object that belongs to the machine — a local group, a local account, the
/// machine's own identity — the collecting account is *how* the data was obtained, not *what* the
/// object belongs to. Keeping it in the identity scope would make the same machine-level group into
/// a different instance for every account the collection happened to run as, and would make the
/// group vanish and reappear as that account changed.
///
/// This is the per-type narrowing that shared change 0005 deliberately left open. The type knows
/// which scope it needs, so the type's adapter decides: the machine-level adapters here ask for the
/// installation, and an adapter for a genuinely per-user object keeps the user segment by taking the
/// scope it was handed.
pub fn installation_scope(collected: &str) -> String {
    let kept: Vec<&str> = collected
        .split('/')
        .filter(|segment| !segment.trim().is_empty())
        .filter(|segment| segment.split(':').next().unwrap_or_default().trim() != "user")
        .collect();
    if kept.is_empty() {
        collected.trim().to_owned()
    } else {
        kept.join("/")
    }
}

/// Builds the identity for a type whose keys are the collection scope plus values taken from the
/// material. A declared key with no value is listed as missing, which is what makes the identity
/// provisional rather than wrong.
pub fn identity(type_id: &str, scope: &str, from: &str, keys: &[(&str, Option<String>)]) -> IdentityCandidate {
    let mut fields = vec![IdentityField { key: "scope".into(), value: scope.into(), from: from.into() }];
    let mut missing = Vec::new();
    for (key, value) in keys {
        match value {
            Some(value) if !value.trim().is_empty() => fields.push(IdentityField { key: (*key).into(), value: value.clone(), from: from.into() }),
            _ => missing.push((*key).to_owned()),
        }
    }
    IdentityCandidate { type_id: type_id.into(), scope: scope.into(), fields, missing, ambiguous: None }
}

/// One object from one item: the shape most sources have.
pub fn object_candidate(
    input: &EvidenceInput,
    context: &AdapterContext,
    identity: IdentityCandidate,
    fields: Vec<FieldCandidate>,
) -> AdapterResult {
    let mut result = AdapterResult::adapted();
    result.observations.push(ObservationCandidate {
        identity,
        fields,
        observed_at: input.observed_at.clone(),
        evidence: vec![input.evidence.clone()],
        complete: context.complete,
    });
    result
}

/// Builds a result from several candidates, or says there was nothing to build from.
pub fn observations(mut candidates: Vec<ObservationCandidate>) -> AdapterResult {
    if candidates.is_empty() {
        return AdapterResult::not_an_object("the material produced no candidate");
    }
    let mut result = AdapterResult::adapted();
    result.observations.append(&mut candidates);
    result
}
