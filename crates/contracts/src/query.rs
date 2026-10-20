//! R05 frozen query contract.
//!
//! Fixes the request and response shapes for the object views so the frontend can be built against
//! them without guessing a backend field, and so a mock service and the real service can be checked
//! against the same structure. Actual queries, indexes and page integration are S22–S25.
//!
//! Two rules are visible in the types rather than left to documentation:
//!
//! * every response states its own status, so "no rows" is never confused with "not supported",
//!   "denied" or "support pruned";
//! * counts are reported separately per kind, so an object count and an evidence count can never
//!   be added together by accident.
use crate::adapter::EvidenceRef;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Page size bounds. A request above the maximum is clamped, not rejected, and the applied limit is
/// echoed back so a client can tell what it actually got.
pub const DEFAULT_PAGE: usize = 50;
pub const MAX_PAGE: usize = 200;
/// Lightweight timeline pages have a separate row and encoded-byte budget.
pub const MAX_TIMELINE_CARDS: usize = 800;
pub const TIMELINE_CARD_BYTES: usize = 4 * 1024 * 1024;

// ---------- requests ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstanceSort {
    NameAsc,
    NameDesc,
    ChangedDesc,
    SeenDesc,
    TypeAsc,
}

/// The lifecycle values a client may filter on. `active` and `not_observed` are the default set;
/// everything else is history and must be asked for explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleFilter {
    Active,
    NotObserved,
    Ended,
    Removed,
    Any,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstanceFilter {
    /// Append-only pages already retain the first page's totals. Callers may omit repeated
    /// related-evidence counts without changing the identity or ordering of the page walk.
    #[serde(default = "default_true")]
    pub include_counts: bool,
    /// Exact number of matching objects, independently of the more expensive related-material
    /// counts. Omitted by default for existing callers; append pages normally need neither.
    #[serde(default)]
    pub include_instance_total: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default)]
    pub lifecycle: Option<LifecycleFilter>,
    #[serde(default = "default_sort")]
    pub sort: InstanceSort,
    /// Opaque cursor from the previous page. Absent means the first page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
}
fn default_sort() -> InstanceSort {
    InstanceSort::NameAsc
}
fn default_limit() -> usize {
    DEFAULT_PAGE
}
impl Default for InstanceFilter {
    fn default() -> Self {
        Self {
            include_counts: true, include_instance_total: false, category: None, type_id: None, search: None, scope: None, identity_status: None,
            coverage: None, source: None, lifecycle: None, sort: default_sort(), cursor: None, limit: default_limit(),
        }
    }
}

/// The filters a cross-object timeline accepts.
///
/// It is about what happened rather than what is, so it orders by when things happened and it can
/// include actions that belong to no object. An action that could not be attributed is still
/// something that happened, and hiding it would make the timeline read as complete when it is not.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionFilter {
    /// Only omit fields unused by cards; full record remains available by detail_id.
    #[serde(default)]
    pub cards_only: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// An object the caller is asking about; absent means every object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
    #[serde(default = "default_true")]
    pub include_unassigned: bool,
    /// Routine refreshes stay available in object history and can be explicitly included here.
    #[serde(default)]
    pub include_routine: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
}
fn default_true() -> bool {
    true
}
impl Default for ActionFilter {
    fn default() -> Self {
        Self { cards_only: false, action_id: None, category: None, type_id: None, source: None, instance: None, include_unassigned: true, include_routine: false, since: None, cursor: None, limit: default_limit() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "query", rename_all = "snake_case")]
pub enum QueryRequest {
    TypeCatalog,
    /// Every action, newest first, across objects. `InstanceActions` answers "what happened to this
    /// one"; this answers "what happened", which is the timeline.
    ActionTimeline(ActionFilter),
    ActionDetail { action_row: i64 },
    InstanceList(InstanceFilter),
    InstanceDetail { instance: String },
    InstanceActions { instance: String, #[serde(default, skip_serializing_if = "Option::is_none")] cursor: Option<String>, #[serde(default = "default_limit")] limit: usize },
    EvidenceDetail { evidence: EvidenceRef },
    SourceCoverage { #[serde(default, skip_serializing_if = "Option::is_none")] source: Option<String> },
    /// Original coverage/configuration readings and resource samples, paged by retained evidence.
    SourceReadings {
        #[serde(default)] source: Option<String>,
        #[serde(default)] cursor: Option<String>,
        #[serde(default = "default_limit")] limit: usize,
    },
    ProjectionStatus,
}

// ---------- response ----------

/// Every state the frontend must render. `Empty` is a successful query with no rows; the others
/// each mean something different and must not be collapsed into it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseStatus {
    Loaded,
    Empty,
    /// The requested type, view or capability is not implemented. Distinct from "no rows".
    Unsupported,
    PermissionDenied,
    /// Some sources answered and some did not. `warnings` says which.
    Partial,
    /// The subject is known but all of its supporting evidence was removed by retention.
    EvidencePruned,
    VersionMismatch,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Warning {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
}

/// Counts are reported per kind and never summed across kinds.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Counts {
    pub instances: u64,
    pub source_records: u64,
    pub actions: u64,
    pub evidence: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Page {
    /// Cursor for the next page. Absent means this is the last page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    /// The limit actually applied after clamping.
    pub limit: usize,
    /// Identifies the query the cursor belongs to. Re-issuing a cursor from a different watermark is
    /// rejected, which is what keeps paging stable while new data arrives.
    pub watermark: String,
    pub has_more: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConflictView {
    pub value: Value,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_time: Option<String>,
}

/// A field as the frontend receives it: label, kind and status all come from the backend, so the UI
/// never has to infer meaning from a raw payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldView {
    pub field_id: String,
    pub label: String,
    /// identity | configuration | state
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    pub value_type: String,
    /// A value_status from the catalog.
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_time: Option<String>,
    pub observed_at: String,
    pub derivation: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflicts: Vec<ConflictView>,
}

/// How much projection work a source's material is waiting on.
///
/// It sits with the source's coverage because the health page has to keep the two failures apart:
/// a source that could not be read is a different problem from material that arrived and could not
/// be turned into objects, and neither number means anything without the other beside it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectionCounts {
    pub pending: u64,
    pub failed: u64,
    pub done: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoverageView {
    pub source: String,
    pub scope: String,
    pub configured: bool,
    /// success | partial | denied | missing | failed | never
    pub last_result: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_success_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_attempt_at: Option<String>,
    /// How much of this source's history is accounted for.
    ///
    /// `catching_up` — a backfill is still walking the retained log towards its end.
    /// `caught_up`   — the read reached the end of what the log still holds.
    /// `gap`         — a known hole: a declared gap, a failed collection, or an interval the item
    ///                 spent switched off. It outlives the recovery that follows it, because
    ///                 resuming collection does not mean the missing interval was observed.
    /// `source_missing` — the source itself is unavailable.
    /// `history_pruned` — retention removed the history this source had.
    /// `unknown`     — nothing has been established about this source's coverage yet.
    ///
    /// `caught_up` is the only one that must never be inferred from a successful collection alone:
    /// see the note in `docs/refactor/acceptance/S32-switch.md` about not substituting the latest
    /// checkpoint for an older log epoch.
    pub history_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// What this source's material is waiting on in the projection. A backlog is not a failure; a
    /// failed count is, and the page reports them separately.
    #[serde(default)]
    pub projection: ProjectionCounts,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstanceSummary {
    /// Absent while the identity is provisional and no id has been allocated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<u64>,
    /// Always present, so a reference works before an id exists.
    pub alias: String,
    pub type_id: String,
    pub category: String,
    pub type_name: String,
    pub display_name: String,
    /// Optional readable presentation (revisions 1 and 2). Missing on older backends; never an identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation: Option<Value>,
    /// Optional observation provenance, not an identity or proof of current availability.
    /// observation | event_only | unknown. Older backends omit it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record_basis: Option<String>,
    pub scope: String,
    /// confirmed | provisional | ambiguous
    pub identity_status: String,
    /// active | not_observed | ended | removed
    pub lifecycle: String,
    pub key_facts: Vec<FieldView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seen_at: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub coverage: Vec<CoverageView>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionView {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_record: Option<Value>,
    /// Optional event-specific labels and attribution checks. Unknown or absent context must not
    /// turn a stored association into a verified one, or a missing name into a factual claim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<Value>,
    pub action_id: String,
    pub title: String,
    /// A result_semantics value. `started` is never rendered as a success.
    pub result: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    /// exact | interval | observation_only
    pub precision: String,
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
    /// True when the time shown is a fallback, so the UI can mark it rather than imply precision.
    pub time_derived: bool,
    pub evidence: Vec<EvidenceRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unassigned_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BindingView {
    pub source: String,
    pub item_id: String,
    pub scope: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_record_id: Option<String>,
    pub valid_from: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_to: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstanceDetail {
    pub summary: InstanceSummary,
    pub fields: Vec<FieldView>,
    pub actions_preview: Vec<ActionView>,
    pub bindings: Vec<BindingView>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceView {
    pub reference: EvidenceRef,
    /// available | pruned
    pub state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pruned_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retained_from: Option<String>,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_time: Option<String>,
    pub observed_at: String,
    pub ingested_at: String,
    /// Whether the reader could decode the original material.
    pub decoding: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub problems: Vec<String>,
    /// False once the raw material has been pruned; the summary may still be readable.
    pub raw_available: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectionStatusView {
    pub projection: String,
    pub rule_version: String,
    pub schema_version: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watermark: Option<String>,
    pub pending: u64,
    pub failed: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeCatalogEntry {
    pub type_id: String,
    pub category: String,
    pub name: String,
    /// False when no source feeds this type yet; the UI must not show it as an empty collection.
    pub supplied_by_source: bool,
}

/// The response envelope. `mock` is set only by the development mock service and is absent from
/// every real response, so a mock can never be mistaken for live data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryResponse {
    pub contract_version: u32,
    pub catalog_version: String,
    pub status: ResponseStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<Warning>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<Page>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counts: Option<Counts>,
    /// Exact filtered object total when requested separately. Absence means unrequested or an
    /// older backend, never zero related records.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance_total: Option<u64>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub mock: bool,
}

impl QueryResponse {
    pub fn empty(catalog_version: &str) -> Self {
        Self { contract_version: 1, catalog_version: catalog_version.into(), status: ResponseStatus::Empty, data: None, warnings: Vec::new(), page: None, counts: None, instance_total: None, mock: false }
    }
    /// A response that a client can tell apart from "no rows".
    pub fn rejected(status: ResponseStatus, code: &str, message: &str, catalog_version: &str) -> Self {
        Self {
            contract_version: 1,
            catalog_version: catalog_version.into(),
            status,
            data: None,
            warnings: vec![Warning { code: code.into(), message: message.into(), at: None }],
            page: None,
            counts: None,
            instance_total: None,
            mock: false,
        }
    }
}

/// The structure a mock response and a real response are both checked against.
pub fn response_shape() -> Value {
    json!({
        "contract_version": 1,
        "presentation_revision": 2,
        "presentation_extensions": {
            "instance_summary.record_basis": {
                "optional": true,
                "values": ["observation", "event_only", "unknown"]
            },
            "instance_summary.presentation": {
                "optional": true,
                "fields": ["name", "description", "name_status", "basis", "dictionary_id", "dictionary_revision", "dictionary_entry_id", "match_basis", "technical_key", "references"],
                "name_status": ["source_provided", "not_provided", "dictionary_matched"]
            },
            "action_view.context": {
                "optional": true,
                "fields": ["type_id", "type_name", "display_name", "key_facts", "presentation", "subject_label", "subject_detail", "subject_basis", "field_label", "program_label", "logon_type", "attribution_status", "attribution_reason", "stored_instance_id", "classification_status", "stored_action_id"],
                "subject_basis": ["event_evidence", "current_object_facts", "unavailable"],
                "attribution_status": ["verified", "unverified", "mismatch", "unassigned", "stored"],
                "classification_status": ["unverified"]
            },
            "compatibility": "Optional display-only additions; missing extensions retain version 1 behavior, unknown extension fields may be ignored. Protected values must not become labels, and unverified associations must not become verified object links."
        },
        "catalog_version": crate::model::catalog_version(),
        "status": ["loaded", "empty", "unsupported", "permission_denied", "partial", "evidence_pruned", "version_mismatch"],
        "sorts": ["name_asc", "name_desc", "changed_desc", "seen_desc", "type_asc"],
        "lifecycle": ["active", "not_observed", "ended", "removed"],
        "lifecycle_filter": ["active", "not_observed", "ended", "removed", "any"],
        "identity_status": ["confirmed", "provisional", "ambiguous"],
        "field_kind": ["identity", "configuration", "state"],
        "value_status": ["known", "not_provided", "unreadable", "not_applicable", "conflicting", "masked", "evidence_pruned"],
        "derivation": ["source_provided", "decoded", "snapshot_comparison", "rule"],
        "time_precision": ["exact", "interval", "observation_only"],
        "action_result": ["succeeded", "failed", "started", "ended", "blocked", "observed", "unknown"],
        "coverage_result": ["success", "partial", "denied", "missing", "failed", "never"],
        "coverage_history": ["catching_up", "caught_up", "gap", "source_missing", "history_pruned", "unknown"],
        "evidence_state": ["available", "pruned"],
        "queries": ["type_catalog", "instance_list", "instance_detail", "instance_actions", "action_timeline", "action_detail", "evidence_detail", "source_coverage", "source_readings", "projection_status"],
        "page": {"default": DEFAULT_PAGE, "maximum": MAX_PAGE},
        "timeline_cards": {"cards_only_default":false,"maximum":MAX_TIMELINE_CARDS,"maximum_data_bytes":TIMELINE_CARD_BYTES,"detail_query":"action_detail","detail_key":"detail_id"},
        "instance_list_counts": {"include_counts_default":true,"include_instance_total_default":false,"instance_total_optional":true,"scope":"same filter and cursor window; unrelated-material counts are omitted, never zero-filled"},
        "default_visibility": "active and not_observed instances are returned by default; ended and removed require an explicit lifecycle filter; provisional instances are included and flagged",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_round_trip_and_defaults_are_applied() {
        let filter: InstanceFilter = serde_json::from_str("{}").unwrap();
        assert_eq!(filter.limit, DEFAULT_PAGE);
        assert_eq!(filter.sort, InstanceSort::NameAsc);
        assert!(filter.cursor.is_none());
        assert!(filter.include_counts);
        assert!(!filter.include_instance_total);

        let request: QueryRequest = serde_json::from_str(r#"{"query":"instance_list","limit":10}"#).unwrap();
        match request {
            QueryRequest::InstanceList(filter) => assert_eq!(filter.limit, 10),
            other => panic!("expected an instance list, got {other:?}"),
        }
        let serialised = serde_json::to_string(&QueryRequest::InstanceList(InstanceFilter::default())).unwrap();
        assert!(serialised.contains("\"query\":\"instance_list\""), "{serialised}");
        // Defaults are omitted so a stored or logged request stays readable.
        assert!(!serialised.contains("cursor"), "{serialised}");
    }

    #[test]
    fn an_empty_result_and_an_unsupported_view_are_different_responses() {
        let empty = QueryResponse::empty("1.0.0");
        let unsupported = QueryResponse::rejected(ResponseStatus::Unsupported, "unsupported_type", "该类型尚未接入来源", "1.0.0");
        assert_ne!(empty.status, unsupported.status);
        assert!(unsupported.warnings[0].message.contains("尚未接入"));
        assert!(!serde_json::to_string(&empty).unwrap().contains("warnings"));
    }

    #[test]
    fn a_mock_response_is_marked_and_a_real_one_is_not() {
        let real = QueryResponse::empty("1.0.0");
        assert!(!serde_json::to_string(&real).unwrap().contains("mock"), "a real response must not carry the mock marker");
        let mock = QueryResponse { mock: true, ..QueryResponse::empty("1.0.0") };
        assert_eq!(serde_json::to_string(&mock).unwrap().contains("\"mock\":true"), true);
    }

    #[test]
    fn counts_are_never_collapsed_into_one_number() {
        let counts = Counts { instances: 3, source_records: 9, actions: 4, evidence: 12 };
        let value = serde_json::to_value(&counts).unwrap();
        for key in ["instances", "source_records", "actions", "evidence"] {
            assert!(value.get(key).is_some(), "counts must report {key} separately");
        }
    }

    #[test]
    fn separate_object_total_is_optional_and_does_not_invent_related_counts() {
        let mut answer=QueryResponse::empty("1.0.0");
        assert!(serde_json::to_value(&answer).unwrap().get("instance_total").is_none());
        answer.instance_total=Some(0);
        let value=serde_json::to_value(&answer).unwrap();
        assert_eq!(value["instance_total"],0);
        assert!(value.get("counts").is_none());
        assert_eq!(serde_json::from_value::<QueryResponse>(value).unwrap(),answer);
        let request:QueryRequest=serde_json::from_value(json!({"query":"instance_list","include_counts":false,"include_instance_total":true})).unwrap();
        let QueryRequest::InstanceList(filter)=request else{panic!("wrong query")};
        assert!(!filter.include_counts);assert!(filter.include_instance_total);
    }

    #[test]
    fn the_shape_declares_the_bounds_and_the_default_visibility() {
        let shape = response_shape();
        assert_eq!(shape["contract_version"], 1);
        assert_eq!(shape["presentation_revision"], 2);
        assert_eq!(shape["presentation_extensions"]["instance_summary.presentation"]["optional"], true);
        assert_eq!(shape["presentation_extensions"]["action_view.context"]["optional"], true);
        assert_eq!(shape["page"]["default"], DEFAULT_PAGE);
        assert_eq!(shape["page"]["maximum"], MAX_PAGE);
        assert_eq!(shape["status"].as_array().unwrap().len(), 7);
        assert!(shape["default_visibility"].as_str().unwrap().contains("provisional"));
    }

    #[test]
    fn optional_presentation_round_trips_without_requiring_it_from_older_backends() {
        let legacy_instance = json!({"alias":"task#1","type_id":"startup.scheduled_task","category":"startup","type_name":"计划任务","display_name":"示例任务","scope":"install","identity_status":"confirmed","lifecycle":"active","key_facts":[]});
        let mut instance: InstanceSummary = serde_json::from_value(legacy_instance.clone()).unwrap();
        assert!(instance.presentation.is_none());
        assert!(instance.record_basis.is_none());
        assert_eq!(serde_json::to_value(&instance).unwrap(), legacy_instance);
        let display = json!({"name":"示例任务","name_status":"source_provided","basis":"object_facts","future_optional":{"value":"preserved"}});
        instance.presentation = Some(display.clone());
        instance.record_basis = Some("event_only".into());
        let extended_instance = serde_json::to_value(&instance).unwrap();
        assert_eq!(serde_json::from_value::<InstanceSummary>(extended_instance.clone()).unwrap().presentation, Some(display));
        assert_eq!(serde_json::from_value::<InstanceSummary>(extended_instance.clone()).unwrap().record_basis.as_deref(), Some("event_only"));

        let legacy_action = json!({"action_id":"startup.task_started","title":"任务开始运行","result":"started","precision":"exact","observed_at":"2026-10-12T00:00:00Z","time_derived":false,"evidence":[]});
        let mut action: ActionView = serde_json::from_value(legacy_action.clone()).unwrap();
        assert!(action.context.is_none());
        assert_eq!(serde_json::to_value(&action).unwrap(), legacy_action);
        let context = json!({"subject_label":null,"subject_basis":"unavailable","attribution_status":"unverified","stored_instance_id":42,"future_optional":{"value":"preserved"}});
        action.context = Some(context.clone());
        let extended_action = serde_json::to_value(&action).unwrap();
        assert_eq!(serde_json::from_value::<ActionView>(extended_action.clone()).unwrap().context, Some(context));

        // Version 1 consumers that know only existing fields continue to decode either generation.
        #[derive(Deserialize)]
        struct LegacyInstance { display_name: String }
        #[derive(Deserialize)]
        struct LegacyAction { action_id: String }
        assert_eq!(serde_json::from_value::<LegacyInstance>(extended_instance).unwrap().display_name, "示例任务");
        assert_eq!(serde_json::from_value::<LegacyAction>(extended_action).unwrap().action_id, "startup.task_started");
    }
}
