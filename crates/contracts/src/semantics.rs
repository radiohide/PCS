//! R04 frozen fact, action, time and retention semantics.
//!
//! This module is the single place these decisions are written down in executable form, so no
//! workstream has to invent its own meaning for a missing value, a time or a duplicate action.
//! It contains decision rules only; selecting, cleaning and recovering are S07/S08 implementations.
use crate::adapter::{ActionCandidate, ActionTarget, EvidenceRef, FieldKind, TimePrecision, ValueStatus};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------- time ----------

/// Which clock a row is being ordered by. Each choice names the anchor it actually reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimelineSort {
    /// When the thing happened, as far as the source states it.
    Occurrence,
    /// When PCScope saw it.
    Observation,
    /// When it was written to the store.
    Ingestion,
}

/// The three times kept for every fact and action. They are never collapsed into one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeFacts {
    pub precision: TimePrecision,
    pub occurred_at: Option<String>,
    pub interval_from: Option<String>,
    pub interval_until: Option<String>,
    pub observed_at: String,
    pub ingested_at: String,
}

/// The time a sort reads, and whether it is the real event time. A fallback must be visible in the
/// UI: showing an observation time in an occurrence column would claim a precision we do not have.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SortAnchor {
    pub time: String,
    pub derived: bool,
    pub basis: String,
}

/// Freezes which time each ordering uses.
pub fn sort_anchor(sort: TimelineSort, facts: &TimeFacts) -> SortAnchor {
    match sort {
        TimelineSort::Ingestion => SortAnchor { time: facts.ingested_at.clone(), derived: false, basis: "入库时间".into() },
        TimelineSort::Observation => SortAnchor { time: facts.observed_at.clone(), derived: false, basis: "观察时间".into() },
        TimelineSort::Occurrence => match facts.precision {
            // An interval is ordered by its end: that is the first moment the change is known to be
            // in effect, and it never pretends to be the moment it happened.
            TimePrecision::Interval => SortAnchor {
                time: facts.interval_until.clone().unwrap_or_else(|| facts.observed_at.clone()),
                derived: true,
                basis: "变化区间上界；精确发生时间未知".into(),
            },
            TimePrecision::Exact => SortAnchor {
                time: facts.occurred_at.clone().unwrap_or_else(|| facts.observed_at.clone()),
                derived: facts.occurred_at.is_none(),
                basis: if facts.occurred_at.is_some() { "来源记录的精确发生时间".into() } else { "缺少发生时间，退回观察时间".into() },
            },
            // Nothing about when it happened is known, so the observation time is used and marked.
            TimePrecision::ObservationOnly => SortAnchor {
                time: facts.observed_at.clone(),
                derived: true,
                basis: "仅观察时间；来源未提供发生时间".into(),
            },
        },
    }
}

/// Canonical stored form: UTC, millisecond precision. Fixed width is what makes text comparison
/// equal to time comparison, which the store relies on for its range queries.
pub fn canonical_time(raw: &str) -> Result<String, String> {
    let text = raw.trim();
    let Some(body) = text.strip_suffix('Z') else {
        return Err("时间必须是 UTC（以 Z 结尾）；带偏移的时间应在采集层转换后再入库".into());
    };
    let Some((date, rest)) = body.split_once('T') else {
        return Err(format!("时间不是 RFC 3339：{raw}"));
    };
    let date_parts: Vec<&str> = date.split('-').collect();
    if date_parts.len() != 3 || date_parts[0].len() != 4 || date_parts[1].len() != 2 || date_parts[2].len() != 2 {
        return Err(format!("日期部分不是 YYYY-MM-DD：{raw}"));
    }
    let (clock, fraction) = match rest.split_once('.') {
        Some((clock, fraction)) => (clock, Some(fraction)),
        None => (rest, None),
    };
    let clock_parts: Vec<&str> = clock.split(':').collect();
    if clock_parts.len() != 3 || clock_parts.iter().any(|part| part.len() != 2) {
        return Err(format!("时间部分不是 HH:MM:SS：{raw}"));
    }
    let digits = date_parts.iter().chain(clock_parts.iter()).all(|part| part.chars().all(|c| c.is_ascii_digit()));
    if !digits {
        return Err(format!("时间含非数字字符：{raw}"));
    }
    // Fractional seconds beyond milliseconds are truncated, not rounded: the stored value must
    // never be later than what the source actually stated.
    let fraction = match fraction {
        None => "000".to_owned(),
        Some(value) if value.chars().all(|c| c.is_ascii_digit()) => {
            let mut value = value.to_owned();
            value.truncate(3);
            while value.len() < 3 {
                value.push('0');
            }
            value
        }
        Some(value) => return Err(format!("小数秒含非数字字符：{value}")),
    };
    Ok(format!("{date}T{clock}.{fraction}Z"))
}

/// Time comparison over canonical values. Equal-width canonical text orders the same way as time,
/// so this is a plain comparison — and a stable tie-break is the caller's second sort key.
pub fn compare_canonical(left: &str, right: &str) -> std::cmp::Ordering {
    left.cmp(right)
}

// ---------- scope ----------

/// The scope an object belongs to, out of the scope a collection happened to cover.
///
/// A collected scope names the identity context first — `machine:`, `install:`, `user:`, `volume:`,
/// `boot:` — and then how the collection was cut up: `schema:`, `context:`, `shard:`. Two sources
/// describing one object differ only in the second part. Keeping that part in the identity scope
/// turns one object into several instances the moment a second source looks at it: the base task
/// inventory and the detailed task configuration carry `context:/shard:0` and `context:task-detail`,
/// and nothing about the task differs between them.
///
/// So the identity scope is the collected scope with the collection segments removed. What is left
/// is what the object belongs to, which is the question identity asks. The per-type scope policy in
/// the catalog (`安装作用域`, `用户作用域`, …) names this same distinction in prose; deciding to
/// narrow it further per type is a separate question and is not decided here.
///
/// An empty or unrecognisable scope is returned unchanged: inventing a scope is worse than keeping
/// an opaque one, because an invented one silently merges objects that were never shown to be the
/// same.
pub fn business_scope(collected: &str) -> String {
    let kept: Vec<&str> = collected
        .split('/')
        .filter(|segment| !segment.trim().is_empty())
        .filter(|segment| !matches!(segment.split(':').next().unwrap_or_default().trim(), "schema" | "context" | "shard"))
        .collect();
    if kept.is_empty() {
        collected.trim().to_owned()
    } else {
        kept.join("/")
    }
}

// ---------- missing values ----------

/// How informative an absence is. Used only to choose which absence to *show*; it never turns an
/// absence into a value.
fn absence_rank(status: ValueStatus) -> Option<u8> {
    Some(match status {
        // A value existed and its evidence was pruned: we know more than for an unreadable field.
        ValueStatus::EvidencePruned => 0,
        ValueStatus::Unreadable => 1,
        ValueStatus::Conflicting => 2,
        ValueStatus::NotProvided => 3,
        ValueStatus::NotApplicable => 4,
        ValueStatus::Known | ValueStatus::Masked => return None,
    })
}

// ---------- current value ----------

/// One observation of one field, from one source. This is the input to the selection rule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FactObservation {
    pub source: String,
    pub value: Option<Value>,
    pub value_type: String,
    pub status: ValueStatus,
    /// Time the source attributes to the value. Falls back to `observed_at` when absent.
    pub source_time: Option<String>,
    pub observed_at: String,
    pub evidence: Vec<EvidenceRef>,
}

/// Whether an observation states a value at all.
///
/// A missing value is not a value. An absence is informative — it decides which absence to *show*,
/// and it can make the current value absent — but it never takes part in comparing one value
/// against another. Keeping the distinction in one place is what stops a caller from deciding for
/// itself that "未提供" is a value and reporting a change no source observed.
pub fn states_a_value(observation: &FactObservation) -> bool {
    absence_rank(observation.status).is_none()
}

/// The time an observation takes effect: the time the source attributes to the value, falling back
/// to when PCScope saw it. §3 orders by this and never by insertion order, so every caller that
/// compares two observations has to agree on what it means — hence one definition, not a copy.
pub fn effective_time(observation: &FactObservation) -> String {
    canonical_time(observation.source_time.as_deref().unwrap_or(&observation.observed_at))
        .unwrap_or_else(|_| observation.observed_at.clone())
}

/// What the current value of a field is, or why it is not a single value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CurrentValue {
    Known {
        value: Value,
        /// Every source that agrees on this value, not just the first one found.
        sources: Vec<String>,
        effective_time: String,
        observed_at: String,
    },
    /// Sources disagree at the same effective time and no declared rule resolves it.
    Conflict { candidates: Vec<FactObservation>, basis: String },
    /// No source states a value. The most informative absence is reported with its reason.
    Absent { status: ValueStatus, reason: String },
    /// The input cannot be used as a stored value at all.
    Refused { reason: String },
}

/// Chooses the current value.
///
/// * effective time decides first — never insertion order, so a late-arriving older observation
///   cannot roll the current state back;
/// * a tie is not broken by source priority unless a rule declares one, so an unresolvable
///   disagreement is kept as a conflict rather than silently picked;
/// * a display mask is refused as a value.
pub fn select_current(observations: &[FactObservation]) -> CurrentValue {
    for observation in observations {
        if observation.value.as_ref().is_some_and(is_mask_sentinel) {
            return CurrentValue::Refused {
                reason: format!(
                    "来源 {} 提交了显示遮罩作为值；遮罩是显示转换，任何情况下都不得作为持久值或参与比较",
                    observation.source
                ),
            };
        }
    }
    let mut valued: Vec<&FactObservation> = observations.iter().filter(|o| states_a_value(o)).collect();
    if valued.is_empty() {
        return match observations.iter().min_by_key(|o| absence_rank(o.status).unwrap_or(9)) {
            Some(observation) => CurrentValue::Absent {
                status: observation.status,
                reason: format!("没有任何来源给出该字段的值；{} 的报告最具体", observation.source),
            },
            None => CurrentValue::Absent { status: ValueStatus::NotProvided, reason: "没有任何观察".into() },
        };
    }
    valued.sort_by(|a, b| effective_time(b).cmp(&effective_time(a)));
    let newest = effective_time(valued[0]);
    let at_newest: Vec<&FactObservation> = valued.iter().copied().filter(|o| effective_time(o) == newest).collect();
    let first = at_newest[0].value.clone().unwrap_or(Value::Null);
    if at_newest.iter().all(|o| o.value.as_ref().is_some_and(|value|crate::sensitive::equivalent(value,&first))) {
        return CurrentValue::Known {
            value: first,
            sources: at_newest.iter().map(|o| o.source.clone()).collect(),
            effective_time: newest,
            observed_at: at_newest.iter().map(|o| o.observed_at.clone()).max().unwrap_or_default(),
        };
    }
    CurrentValue::Conflict {
        candidates: at_newest.into_iter().cloned().collect(),
        basis: "同一生效时间上来源给出不同值，且没有声明的优先级规则可以消解".into(),
    }
}

/// The display mask must never be stored or compared as if it were data.
pub fn is_mask_sentinel(value: &Value) -> bool {
    value.as_str() == Some(crate::sensitive::MASK)
}

// ---------- change classification ----------

/// The action a field change produces. It is derived from the field's kind, so no caller can
/// choose a stronger wording for a change than the kind supports.
///
/// This is what keeps a task's next-run time from being reported as a human edit: a state field
/// can only produce `observed.state_changed`.
pub fn change_action(field_kind: FieldKind) -> Option<&'static str> {
    match field_kind {
        FieldKind::Configuration => Some("observed.configuration_changed"),
        FieldKind::State => Some("observed.state_changed"),
        // An identity field does not "change": a different identity is a different instance, which
        // is the identity layer's business, not a change action.
        FieldKind::Identity => None,
    }
}

// ---------- action merging ----------

/// Whether two records of the same action are the same action. Merging is opt-in per rule; the
/// default is to keep them separate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum MergeDecision {
    Merge { reason: String },
    KeepSeparate { reason: String },
}

fn target_alias(action: &ActionCandidate) -> Option<String> {
    match &action.target {
        ActionTarget::Assigned { identity } => Some(format!(
            "{}|{}|{}|{}",
            identity.type_id, identity.scope,
            identity.fields.iter().map(|f| format!("{}={}", f.key, f.value)).collect::<Vec<_>>().join(","),
            identity.missing.join(",")
        )),
        ActionTarget::Unassigned { .. } => None,
    }
}

/// Decides whether an incoming record joins an existing action.
///
/// Time proximity alone never merges: two records must agree on the target, the action type and
/// the phase, and one of them must carry a correlation that states they are the same occurrence.
pub fn merge_action(existing: &ActionCandidate, incoming: &ActionCandidate) -> MergeDecision {
    let separate = |reason: &str| MergeDecision::KeepSeparate { reason: reason.to_owned() };
    match (target_alias(existing), target_alias(incoming)) {
        (None, _) | (_, None) => return separate("目标是未归属的，无法断定两条记录指向同一次动作"),
        (Some(left), Some(right)) if left != right => return separate("目标对象不同"),
        _ => {}
    }
    if existing.action_id != incoming.action_id {
        return separate("动作类型不同；不同阶段或不同动作不得归并");
    }
    // Phases are kept apart: a start and an end are two stages, not one success.
    if existing.phase != incoming.phase {
        return separate("阶段不同；开始与结束必须分别保留，不得去重成一条成功事件");
    }
    if existing.precision != incoming.precision {
        return separate("时间精度不同，无法确认是同一次发生");
    }
    let same_moment = match existing.precision {
        TimePrecision::Exact => existing.occurred_at.is_some() && existing.occurred_at == incoming.occurred_at,
        TimePrecision::Interval => existing.interval_from == incoming.interval_from && existing.interval_until == incoming.interval_until,
        TimePrecision::ObservationOnly => false,
    };
    if !same_moment {
        return separate("发生时间或区间不同；不因时间接近而归并");
    }
    MergeDecision::Merge { reason: "目标、动作类型、阶段与发生时间完全一致，视为同一次动作的多份证据".into() }
}

// ---------- absence eligibility ----------

/// Everything that must hold before an instance may be called "not observed" this round.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AbsenceFacts {
    pub snapshot_complete: bool,
    pub item_configured: bool,
    pub collection_succeeded: bool,
    pub instance_scope_covered: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum AbsenceVerdict {
    Eligible { basis: String },
    Ineligible { reason: String },
}

/// Absence is a claim about coverage, not about the object. Every condition is reported separately
/// so the UI can say why it cannot conclude, instead of showing a disappearance.
pub fn absence_eligible(facts: &AbsenceFacts) -> AbsenceVerdict {
    let missing = |reason: &str| AbsenceVerdict::Ineligible { reason: reason.to_owned() };
    if !facts.item_configured {
        return missing("该采集项已停用；停用期间的缺失不构成未发现");
    }
    if !facts.collection_succeeded {
        return missing("本轮采集未成功；读取失败不能当作未发现");
    }
    if !facts.snapshot_complete {
        return missing("本轮快照不完整；不完整快照中的缺失不能当作未发现");
    }
    if !facts.instance_scope_covered {
        return missing("本实例所在范围未被本轮采集覆盖；这是范围变化，不是对象消失");
    }
    AbsenceVerdict::Eligible { basis: "完整且成功的快照覆盖了该实例所在范围，且采集项处于启用状态".into() }
}

// ---------- pruned evidence ----------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "reference", rename_all = "snake_case")]
pub enum ReferenceState {
    Available { evidence: EvidenceRef },
    /// The evidence was removed by retention. The reference still resolves to a stated reason and
    /// a time, never to an empty value.
    Pruned { evidence: EvidenceRef, pruned_at: String, retained_from: Option<String> },
}

/// What a reference reports. A pruned reference is never silently dropped and never returned as
/// available: a conclusion whose support is gone must say so.
pub fn reference_state(evidence: &EvidenceRef, pruned_at: Option<&str>, retained_from: Option<&str>) -> ReferenceState {
    match pruned_at {
        Some(time) => ReferenceState::Pruned {
            evidence: evidence.clone(),
            pruned_at: time.to_owned(),
            retained_from: retained_from.map(str::to_owned),
        },
        None => ReferenceState::Available { evidence: evidence.clone() },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::{ActionOutcome, IdentityCandidate, IdentityField};
    use serde_json::json;

    fn observation(source: &str, value: Option<Value>, status: ValueStatus, source_time: Option<&str>, observed_at: &str) -> FactObservation {
        FactObservation {
            source: source.into(),
            value,
            value_type: "string".into(),
            status,
            source_time: source_time.map(str::to_owned),
            observed_at: observed_at.into(),
            evidence: vec![EvidenceRef::record(1)],
        }
    }
    fn action(action_id: &str, phase: Option<&str>, precision: TimePrecision, occurred_at: Option<&str>) -> ActionCandidate {
        ActionCandidate {
            occurrence_key: None, source_record: None,
            target: ActionTarget::Assigned {
                identity: IdentityCandidate {
                    type_id: "startup.scheduled_task".into(),
                    scope: "install".into(),
                    fields: vec![
                        IdentityField { key: "scope".into(), value: "install".into(), from: "t".into() },
                        IdentityField { key: "path".into(), value: "\\A".into(), from: "t".into() },
                    ],
                    missing: Vec::new(),
                    ambiguous: None,
                },
            },
            action_id: action_id.into(),
            phase: phase.map(str::to_owned),
            result: ActionOutcome::Observed,
            precision,
            occurred_at: occurred_at.map(str::to_owned),
            interval_from: None,
            interval_until: None,
            observed_at: "2026-09-12T00:00:00.000Z".into(),
            field_id: None,
            before: None,
            after: None,
            evidence: vec![EvidenceRef::record(1)],
            actor: None,
            program: None,
        }
    }

    #[test]
    fn canonical_time_is_fixed_width_so_text_order_is_time_order() {
        assert_eq!(canonical_time("2026-09-11T00:00:00Z").unwrap(), "2026-09-11T00:00:00.000Z");
        assert_eq!(canonical_time("2026-09-11T00:00:00.5Z").unwrap(), "2026-09-11T00:00:00.500Z");
        assert_eq!(canonical_time("2026-09-11T00:00:00.123456Z").unwrap(), "2026-09-11T00:00:00.123Z");
        // Without fixed width, "….500Z" would sort before "…Z" although it is later.
        let earlier = canonical_time("2026-09-11T00:00:00Z").unwrap();
        let later = canonical_time("2026-09-11T00:00:00.500Z").unwrap();
        assert_eq!(compare_canonical(&earlier, &later), std::cmp::Ordering::Less);
        assert!(canonical_time("2026-09-11T08:00:00+08:00").is_err(), "offsets must be converted before storing");
        assert!(canonical_time("2026-09-11").is_err());
    }

    #[test]
    fn both_time_formats_the_collectors_actually_emit_canonicalise() {
        // PowerShell's round-trip format, used for source and observation times: seven fractional
        // digits. Truncating rather than rounding keeps the stored time no later than the stated one.
        assert_eq!(canonical_time("2026-09-11T09:30:00.1234567Z").unwrap(), "2026-09-11T09:30:00.123Z");
        // SQLite's strftime millisecond form, used for ingestion times.
        assert_eq!(canonical_time("2026-09-11T09:30:00.123Z").unwrap(), "2026-09-11T09:30:00.123Z");
        // Both forms order consistently once canonical.
        let a = canonical_time("2026-09-11T09:30:00.1234567Z").unwrap();
        let b = canonical_time("2026-09-11T09:30:00.200Z").unwrap();
        assert_eq!(compare_canonical(&a, &b), std::cmp::Ordering::Less);
    }

    #[test]
    fn each_sort_reads_its_own_anchor_and_marks_a_fallback() {
        let facts = TimeFacts {
            precision: TimePrecision::Exact,
            occurred_at: Some("2026-09-11T00:00:00Z".into()),
            interval_from: None,
            interval_until: None,
            observed_at: "2026-09-11T01:00:00Z".into(),
            ingested_at: "2026-09-11T02:00:00Z".into(),
        };
        assert_eq!(sort_anchor(TimelineSort::Occurrence, &facts).time, "2026-09-11T00:00:00Z");
        assert!(!sort_anchor(TimelineSort::Occurrence, &facts).derived);
        assert_eq!(sort_anchor(TimelineSort::Observation, &facts).time, "2026-09-11T01:00:00Z");
        assert_eq!(sort_anchor(TimelineSort::Ingestion, &facts).time, "2026-09-11T02:00:00Z");
        let no_occurrence = TimeFacts { occurred_at: None, ..facts.clone() };
        let anchor = sort_anchor(TimelineSort::Occurrence, &no_occurrence);
        assert!(anchor.derived, "a fallback to observation time must be marked");
        let interval = TimeFacts { precision: TimePrecision::Interval, interval_until: Some("2026-09-11T00:30:00Z".into()), ..facts };
        assert_eq!(sort_anchor(TimelineSort::Occurrence, &interval).time, "2026-09-11T00:30:00Z");
    }

    #[test]
    fn a_late_arriving_older_observation_never_becomes_current() {
        // The newer value was stored first, then an older log arrived afterwards.
        let observations = vec![
            observation("pcscope.startup.tasks", Some(json!("command B")), ValueStatus::Known, Some("2026-09-11T10:05:00Z"), "2026-09-11T10:05:00Z"),
            observation("pcscope.eventlog.system", Some(json!("command A")), ValueStatus::Known, Some("2026-09-11T10:00:00Z"), "2026-09-11T10:06:00Z"),
        ];
        match select_current(&observations) {
            CurrentValue::Known { value, sources, .. } => {
                assert_eq!(value, json!("command B"));
                assert_eq!(sources, vec!["pcscope.startup.tasks"]);
            }
            other => panic!("expected the newer value, got {other:?}"),
        }
    }

    #[test]
    fn disagreeing_sources_at_the_same_time_are_kept_as_a_conflict() {
        let observations = vec![
            observation("source.a", Some(json!("x")), ValueStatus::Known, Some("2026-09-11T10:00:00Z"), "2026-09-11T10:00:00Z"),
            observation("source.b", Some(json!("y")), ValueStatus::Known, Some("2026-09-11T10:00:00Z"), "2026-09-11T10:00:00Z"),
        ];
        match select_current(&observations) {
            CurrentValue::Conflict { candidates, .. } => assert_eq!(candidates.len(), 2),
            other => panic!("expected a conflict, got {other:?}"),
        }
        // Agreement at the same time is one value citing both sources.
        let agreeing = vec![
            observation("source.a", Some(json!("x")), ValueStatus::Known, Some("2026-09-11T10:00:00Z"), "2026-09-11T10:00:00Z"),
            observation("source.b", Some(json!("x")), ValueStatus::Known, Some("2026-09-11T10:00:00Z"), "2026-09-11T10:00:00Z"),
        ];
        match select_current(&agreeing) {
            CurrentValue::Known { sources, .. } => assert_eq!(sources.len(), 2),
            other => panic!("expected agreement, got {other:?}"),
        }
    }

    #[test]
    fn a_display_mask_is_refused_as_a_value() {
        let observations = vec![observation("source.a", Some(json!(crate::sensitive::MASK)), ValueStatus::Masked, Some("2026-09-11T10:00:00Z"), "2026-09-11T10:00:00Z")];
        match select_current(&observations) {
            CurrentValue::Refused { reason } => assert!(reason.contains("遮罩"), "{reason}"),
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn the_most_informative_absence_is_reported_and_never_a_value() {
        let observations = vec![
            observation("source.a", None, ValueStatus::NotProvided, None, "2026-09-11T10:00:00Z"),
            observation("source.b", None, ValueStatus::Unreadable, None, "2026-09-11T10:00:00Z"),
        ];
        match select_current(&observations) {
            CurrentValue::Absent { status, .. } => assert_eq!(status, ValueStatus::Unreadable),
            other => panic!("expected the more informative absence, got {other:?}"),
        }
        let pruned = vec![
            observation("source.a", None, ValueStatus::Unreadable, None, "t"),
            observation("source.b", None, ValueStatus::EvidencePruned, None, "t"),
        ];
        match select_current(&pruned) {
            CurrentValue::Absent { status, .. } => assert_eq!(status, ValueStatus::EvidencePruned),
            other => panic!("expected evidence_pruned to outrank unreadable, got {other:?}"),
        }
    }

    #[test]
    fn a_field_change_can_only_produce_an_observed_action() {
        assert_eq!(change_action(FieldKind::State), Some("observed.state_changed"));
        assert_eq!(change_action(FieldKind::Configuration), Some("observed.configuration_changed"));
        assert_eq!(change_action(FieldKind::Identity), None, "a changed identity is a different instance, not a change action");
        // The catalog must not describe any of these as a source-recorded action: that is what keeps
        // a next-run-time change from being reported as a human edit.
        for kind in [FieldKind::State, FieldKind::Configuration] {
            let action_id = change_action(kind).unwrap();
            let definition = crate::model::action_definition(action_id).expect("registered action");
            assert_ne!(definition["nature"].as_str(), Some("source_recorded"), "{action_id} must not be source-recorded");
        }
    }

    #[test]
    fn actions_merge_only_when_target_type_phase_and_time_all_agree() {
        let base = action("execution.process_started", Some("start"), TimePrecision::Exact, Some("2026-09-11T10:00:00Z"));
        let identical = action("execution.process_started", Some("start"), TimePrecision::Exact, Some("2026-09-11T10:00:00Z"));
        assert!(matches!(merge_action(&base, &identical), MergeDecision::Merge { .. }));

        let other_phase = action("execution.process_started", Some("end"), TimePrecision::Exact, Some("2026-09-11T10:00:00Z"));
        assert!(matches!(merge_action(&base, &other_phase), MergeDecision::KeepSeparate { .. }));

        // One second apart is not the same occurrence.
        let near = action("execution.process_started", Some("start"), TimePrecision::Exact, Some("2026-09-11T10:00:01Z"));
        match merge_action(&base, &near) {
            MergeDecision::KeepSeparate { reason } => assert!(reason.contains("不因时间接近"), "{reason}"),
            other => panic!("expected separate, got {other:?}"),
        }

        let different_action = action("execution.process_stopped", Some("start"), TimePrecision::Exact, Some("2026-09-11T10:00:00Z"));
        assert!(matches!(merge_action(&base, &different_action), MergeDecision::KeepSeparate { .. }));

        let unassigned = ActionCandidate { occurrence_key: None, source_record: None, target: ActionTarget::Unassigned { reason: "no target".into() }, ..base.clone() };
        assert!(matches!(merge_action(&unassigned, &identical), MergeDecision::KeepSeparate { .. }));
    }

    #[test]
    fn absence_needs_complete_configured_and_successful_coverage() {
        let ok = AbsenceFacts { snapshot_complete: true, item_configured: true, collection_succeeded: true, instance_scope_covered: true };
        assert!(matches!(absence_eligible(&ok), AbsenceVerdict::Eligible { .. }));
        for (facts, expect) in [
            (AbsenceFacts { item_configured: false, ..ok.clone() }, "停用"),
            (AbsenceFacts { collection_succeeded: false, ..ok.clone() }, "未成功"),
            (AbsenceFacts { snapshot_complete: false, ..ok.clone() }, "不完整"),
            (AbsenceFacts { instance_scope_covered: false, ..ok.clone() }, "未被本轮采集覆盖"),
        ] {
            match absence_eligible(&facts) {
                AbsenceVerdict::Ineligible { reason } => assert!(reason.contains(expect), "{reason} should mention {expect}"),
                other => panic!("expected ineligible, got {other:?}"),
            }
        }
    }

    #[test]
    fn a_pruned_reference_states_that_it_was_pruned() {
        let evidence = EvidenceRef::record(42);
        assert!(matches!(reference_state(&evidence, None, None), ReferenceState::Available { .. }));
        match reference_state(&evidence, Some("2026-09-12T00:00:00Z"), Some("2026-09-01T00:00:00Z")) {
            ReferenceState::Pruned { pruned_at, retained_from, .. } => {
                assert_eq!(pruned_at, "2026-09-12T00:00:00Z");
                assert_eq!(retained_from.as_deref(), Some("2026-09-01T00:00:00Z"));
            }
            other => panic!("expected pruned, got {other:?}"),
        }
    }

    #[test]
    fn a_business_scope_drops_how_the_collection_was_cut_up() {
        // Two sources describing one task. Only the collection segments differ, so the identity
        // scope has to come out the same or the task becomes two instances.
        assert_eq!(business_scope("machine:m/install:i/schema:1/context:/shard:0"), "machine:m/install:i");
        assert_eq!(business_scope("machine:m/install:i/schema:1/context:task-detail"), "machine:m/install:i");
        assert_eq!(business_scope("machine:m/install:i/schema:1/context:task-detail/shard:0"), "machine:m/install:i");
        // A user-scoped object keeps the user segment.
        let user = "machine:11111111-2222-3333-4444-555555555555/user:S-1-5-21-1001/schema:1/context:/shard:0";
        assert_eq!(business_scope(user), "machine:11111111-2222-3333-4444-555555555555/user:S-1-5-21-1001");
        // A scope that names no identity segments is kept verbatim: an opaque scope is better than
        // an invented one, because an invented one merges objects that were never shown to match.
        assert_eq!(business_scope("fixed drives with drive letters only"), "fixed drives with drive letters only");
        assert_eq!(business_scope(""), "");
    }
}
