//! R02 adapter contract: two structurally different sources through one interface, plus the
//! acceptance demonstrations the readiness plan requires.
//!
//! The two adapters live in `pcscope_contracts::reference` so the contract-case runner (R06) uses
//! the same code instead of a parallel copy.
//!
//! usage: adapter-contract [fixtures-directory]   (default: fixtures)
//!
//! Exits non-zero if any acceptance condition fails, so it can gate a build.
use pcscope_contracts::adapter::*;
use pcscope_contracts::model;
use pcscope_contracts::reference::{read_fixture, ScheduledTaskAdapter, SecurityEventAdapter};
use serde_json::{json, Value};
use std::path::PathBuf;

struct Report {
    failures: Vec<String>,
    checks: Vec<Value>,
}
impl Report {
    fn check(&mut self, name: &str, passed: bool, detail: impl Into<String>) {
        let detail = detail.into();
        if !passed {
            self.failures.push(format!("{name}: {detail}"));
        }
        self.checks.push(json!({"check": name, "passed": passed, "detail": detail}));
    }
}

fn main() -> anyhow::Result<()> {
    let directory: PathBuf = std::env::args().nth(1).unwrap_or_else(|| "fixtures".into()).into();
    let context = AdapterContext {
        catalog_version: model::catalog_version().into(),
        rule_version: model::catalog_fingerprint(),
        machine_id: "11111111-2222-3333-4444-555555555555".into(),
        installation_id: "sss-dev".into(),
        boot_id: None,
        collected_scope: "machine:11111111-2222-3333-4444-555555555555/user:S-1-5-21-1000000000-2000000000-3000000000-1001/schema:1".into(),
        complete: true,
    };
    let tasks = read_fixture(&directory.join("01-baseline.jsonl"))?;
    let events = read_fixture(&directory.join("04-events.jsonl"))?;
    let mut report = Report { failures: Vec::new(), checks: Vec::new() };

    let task_adapter = ScheduledTaskAdapter;
    let event_adapter = SecurityEventAdapter;
    let task_inputs: Vec<&EvidenceInput> = tasks.iter().filter(|i| i.source == "pcscope.startup.tasks").collect();
    let event_inputs: Vec<&EvidenceInput> = events.iter().filter(|i| i.source == "pcscope.eventlog.security").collect();
    let task_results: Vec<AdapterResult> = task_inputs.iter().map(|i| task_adapter.adapt(i, &context)).collect();
    let event_results: Vec<AdapterResult> = event_inputs.iter().map(|i| event_adapter.adapt(i, &context)).collect();
    let observations: usize = task_results.iter().map(|r| r.observations.len()).sum();
    let actions: usize = event_results.iter().map(|r| r.actions.len()).sum();

    report.check(
        "one interface, two structurally different sources",
        task_adapter.adapter_id() != event_adapter.adapter_id() && observations > 0 && actions > 0,
        format!("{} produced {observations} observations; {} produced {actions} actions", task_adapter.adapter_id(), event_adapter.adapter_id()),
    );

    let mut violations = Vec::new();
    for result in task_results.iter().chain(event_results.iter()) {
        for violation in validate(result, &context) {
            violations.push(format!("{} at {}", violation.message, violation.at));
        }
    }
    report.check("candidates validate against the catalog", violations.is_empty(),
        if violations.is_empty() { "no violations".to_string() } else { violations.join("; ") });

    // Unknown fields stay in the raw material and never reach a candidate. `adapt` only receives a
    // shared reference, so this is structural rather than a promise.
    let mut unknown = tasks.iter().find(|i| i.source == "pcscope.startup.tasks").expect("a task fixture").clone();
    unknown.payload["a_field_no_adapter_knows"] = json!({"nested": [1, 2, 3]});
    let before = serde_json::to_string(&unknown.payload)?;
    let with_unknown = task_adapter.adapt(&unknown, &context);
    let after = serde_json::to_string(&unknown.payload)?;
    let leaked = serde_json::to_string(&with_unknown)?.contains("a_field_no_adapter_knows");
    report.check("unknown fields stay in the raw evidence and do not leak into candidates",
        before == after && !leaked,
        format!("raw payload unchanged: {}; present in candidates: {leaked}", before == after));

    let mut sparse = unknown.clone();
    sparse.payload = json!({"path": "\\Fixtures\\Sparse"});
    let sparse_result = task_adapter.adapt(&sparse, &context);
    let enabled = sparse_result.observations[0].fields.iter().find(|f| f.field_id == "enabled").expect("an enabled field");
    report.check("a missing field is recorded absent and carries no value",
        enabled.status == ValueStatus::NotProvided && enabled.value.is_none(),
        format!("status {:?}, value {:?}", enabled.status, enabled.value));

    let first = serde_json::to_string(&task_results)?;
    let second: Vec<AdapterResult> = task_inputs.iter().map(|i| task_adapter.adapt(i, &context)).collect();
    let identical = first == serde_json::to_string(&second)?;
    report.check("the same input yields the same candidates", identical,
        if identical { "byte-identical on re-run".to_string() } else { "differs between runs".to_string() });

    let foreign = EvidenceInput { source: "pcscope.execution.resources".into(), ..unknown.clone() };
    let foreign_result = task_adapter.adapt(&foreign, &context);
    let broken = EvidenceInput { payload: json!({"name": "no path"}), ..unknown.clone() };
    let broken_result = task_adapter.adapt(&broken, &context);
    let unclassifiable = EvidenceInput {
        source: "pcscope.eventlog.security".into(),
        payload: json!({"provider": "示例未知提供程序", "event_id": 999999}),
        ..unknown.clone()
    };
    let unclassified_result = event_adapter.adapt(&unclassifiable, &context);
    report.check("failures and non-objects carry an explicit outcome",
        matches!(foreign_result.outcome, AdapterOutcome::NotAnObject { .. })
            && matches!(broken_result.outcome, AdapterOutcome::Failed { retryable: false, .. })
            && matches!(unclassified_result.outcome, AdapterOutcome::NotAnObject { .. })
            && !foreign_result.complete()
            && !broken_result.complete()
            && unclassified_result.actions.is_empty(),
        format!("not-an-object: {:?}; failed: {:?}; unclassifiable event produced {} candidate(s) and a stated reason",
            foreign_result.outcome, broken_result.outcome, unclassified_result.actions.len()));

    let mut partial = tasks[0].clone();
    partial.source = "pcscope.eventlog.security".into();
    partial.payload = json!({"provider": "Microsoft-Windows-Security-Auditing", "event_id": 4688});
    let partial_result = event_adapter.adapt(&partial, &context);
    let marked = partial_result.actions.iter().any(|a| match &a.target {
        ActionTarget::Assigned { identity } => identity.missing.contains(&"pid".to_string()),
        ActionTarget::Unassigned { .. } => false,
    });
    report.check("an identity the source cannot complete is marked, not invented",
        !partial_result.actions.is_empty() && marked,
        format!("event 4688 without a process id -> missing keys {:?}",
            partial_result.actions.first().map(|a| match &a.target {
                ActionTarget::Assigned { identity } => identity.missing.clone(),
                ActionTarget::Unassigned { reason } => vec![reason.clone()],
            })));

    let mut tampered = task_results[0].clone();
    tampered.observations[0].identity.type_id = "startup.invented_type".into();
    let caught = validate(&tampered, &context);
    report.check("an unregistered type is rejected", caught.iter().any(|v| v.message.contains("not registered")),
        format!("{} violation(s)", caught.len()));

    let schema = contract_schema();
    report.check("the contract schema reports one version and the live catalog",
        schema["contract_version"] == 1 && schema["catalog_version"] == model::catalog_version(),
        format!("contract {} against catalog {}", schema["contract_version"], schema["catalog_version"]));

    let value = json!({
        "contract_version": 1,
        "catalog_version": model::catalog_version(),
        "rule_version": model::catalog_fingerprint(),
        "fixtures": directory.display().to_string(),
        "inputs": {"scheduled_task": task_inputs.len(), "security_event": event_inputs.len()},
        "candidates": {"observations": observations, "actions": actions},
        "checks": report.checks,
    });
    println!("{}", serde_json::to_string_pretty(&value)?);
    if report.failures.is_empty() {
        println!("adapter contract: {} checks passed", value["checks"].as_array().map_or(0, Vec::len));
        return Ok(());
    }
    eprintln!("adapter contract FAILED:");
    for failure in &report.failures {
        eprintln!("  ! {failure}");
    }
    std::process::exit(1);
}
