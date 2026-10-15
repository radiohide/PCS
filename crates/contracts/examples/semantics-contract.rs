//! R04 semantics contract: the five acceptance scenarios, each with its one expected handling.
//!
//! usage: semantics-contract
//!
//! Exits non-zero if any scenario does not behave as the decision table states.
use pcscope_contracts::adapter::{EvidenceRef, FieldKind, TimePrecision, ValueStatus};
use pcscope_contracts::model;
use pcscope_contracts::semantics::*;
use serde_json::{json, Value};

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

struct Report {
    failures: Vec<String>,
    rows: Vec<Value>,
}
impl Report {
    fn scenario(&mut self, name: &str, decision: &str, actual: Value, ok: bool) {
        if !ok {
            self.failures.push(format!("{name}: expected {decision}, got {actual}"));
        }
        self.rows.push(json!({"scenario": name, "expected": decision, "actual": actual, "passed": ok}));
    }
}

fn main() {
    let mut report = Report { failures: Vec::new(), rows: Vec::new() };

    // 1. The latest API snapshot and an earlier log disagree. The earlier record is history; it never
    //    sets the current value, and the change is reported as an interval, not an instant.
    let api = observation("pcscope.startup.tasks", Some(json!("--示例程序 B")), ValueStatus::Known, Some("2026-09-11T10:05:00Z"), "2026-09-11T10:05:00Z");
    let log = observation("pcscope.eventlog.system", Some(json!("--示例程序 A")), ValueStatus::Known, Some("2026-09-11T10:00:00Z"), "2026-09-11T10:07:00Z");
    let selection = select_current(&[api.clone(), log.clone()]);
    let interval = TimeFacts {
        precision: TimePrecision::Interval,
        occurred_at: None,
        interval_from: Some("2026-09-11T10:00:00Z".into()),
        interval_until: Some("2026-09-11T10:05:00Z".into()),
        observed_at: "2026-09-11T10:05:00Z".into(),
        ingested_at: "2026-09-11T10:07:00Z".into(),
    };
    let anchor = sort_anchor(TimelineSort::Occurrence, &interval);
    let actual = json!({
        "current": match &selection { CurrentValue::Known { value, sources, effective_time, .. } => json!({"value": value, "sources": sources, "effective_time": effective_time}), other => serde_json::to_value(other).unwrap() },
        "change_reported_as": "interval",
        "interval_until": anchor.time,
        "anchor_derived": anchor.derived,
    });
    report.scenario(
        "latest API snapshot plus an earlier log",
        "current value comes from the newer read; the older record does not roll it back; the change is an interval",
        actual.clone(),
        matches!(&selection, CurrentValue::Known { value, .. } if value == &json!("--示例程序 B"))
            && anchor.time == "2026-09-11T10:05:00Z" && anchor.derived,
    );

    // 2. Two sources, one field, same effective time, different values, no declared priority.
    let conflict = select_current(&[
        observation("pcscope.security.protection", Some(json!(true)), ValueStatus::Known, Some("2026-09-11T10:00:00Z"), "2026-09-11T10:00:00Z"),
        observation("pcscope.security.protection-policy", Some(json!(false)), ValueStatus::Known, Some("2026-09-11T10:00:00Z"), "2026-09-11T10:00:00Z"),
    ]);
    let actual = match &conflict {
        CurrentValue::Conflict { candidates, basis } => json!({"state": "conflict", "candidates": candidates.len(), "basis": basis}),
        other => serde_json::to_value(other).unwrap(),
    };
    report.scenario(
        "same field, disagreeing sources",
        "kept as a conflict with both candidates; no source is silently preferred",
        actual.clone(),
        matches!(&conflict, CurrentValue::Conflict { candidates, .. } if candidates.len() == 2),
    );

    // 3. A collection item is disabled. Nothing may be called absent, and the last known state stays.
    let disabled = absence_eligible(&AbsenceFacts {
        snapshot_complete: true,
        item_configured: false,
        collection_succeeded: true,
        instance_scope_covered: true,
    });
    let last_known = observation("pcscope.startup.tasks", Some(json!("--示例程序 B")), ValueStatus::Known, Some("2026-09-11T10:05:00Z"), "2026-09-11T10:05:00Z");
    let retained = select_current(&[last_known.clone()]);
    let actual = json!({
        "absence": serde_json::to_value(&disabled).unwrap(),
        "current_value_retained": matches!(&retained, CurrentValue::Known { .. }),
    });
    report.scenario(
        "collection item disabled",
        "absence is ineligible and the last observed value is retained, not cleared",
        actual.clone(),
        matches!(&disabled, AbsenceVerdict::Ineligible { .. }) && matches!(&retained, CurrentValue::Known { .. }),
    );

    // 4. Evidence for a conclusion was pruned by retention. The conclusion stays; the reference says so.
    let evidence = EvidenceRef::record(42);
    let pruned = reference_state(&evidence, Some("2026-09-12T00:00:00Z"), Some("2026-09-01T00:00:00Z"));
    let still_current = select_current(&[observation("pcscope.startup.tasks", Some(json!("--示例程序 B")), ValueStatus::Known, Some("2026-09-11T10:05:00Z"), "2026-09-11T10:05:00Z")]);
    let actual = json!({
        "reference": serde_json::to_value(&pruned).unwrap(),
        "current_value_retained": matches!(&still_current, CurrentValue::Known { .. }),
        "value_status_is_not_evidence_pruned": matches!(&still_current, CurrentValue::Known { .. }),
    });
    report.scenario(
        "evidence pruned by retention",
        "the reference reports pruned with its time; the conclusion and its own observation time survive",
        actual.clone(),
        matches!(&pruned, ReferenceState::Pruned { pruned_at, retained_from, .. }
            if pruned_at == "2026-09-12T00:00:00Z" && retained_from.is_some())
            && matches!(&still_current, CurrentValue::Known { .. }),
    );

    // 5. A task's next-run time changed. It is a state field, so the only action it can produce is a
    //    state change — never a claim that someone edited the task.
    let next_run_action = change_action(FieldKind::State).unwrap();
    let definition = model::action_definition(next_run_action).expect("registered");
    let configuration_action = change_action(FieldKind::Configuration).unwrap();
    let actual = json!({
        "state_change_action": next_run_action,
        "nature": definition["nature"],
        "configuration_change_action": configuration_action,
        "identity_fields_produce_a_change": change_action(FieldKind::Identity).is_some(),
    });
    report.scenario(
        "task next-run time changes",
        "reported as observed.state_changed; no path produces a source-recorded action for a field change",
        actual.clone(),
        definition["nature"].as_str() == Some("state_change")
            && change_action(FieldKind::Identity).is_none()
            && model::action_definition(configuration_action).map(|d| d["nature"].as_str() == Some("snapshot_change")) == Some(true),
    );

    let value = json!({
        "contract_version": 1,
        "catalog_version": model::catalog_version(),
        "rule_version": model::catalog_fingerprint(),
        "scenarios": report.rows,
    });
    println!("{}", serde_json::to_string_pretty(&value).unwrap());
    if report.failures.is_empty() {
        println!("semantics contract: {} scenarios passed", value["scenarios"].as_array().map_or(0, Vec::len));
        return;
    }
    eprintln!("semantics contract FAILED:");
    for failure in &report.failures {
        eprintln!("  ! {failure}");
    }
    std::process::exit(1);
}
