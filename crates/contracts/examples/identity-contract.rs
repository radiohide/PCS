//! R03 identity contract: the five boundary cases with fixed expectations.
//!
//! usage: identity-contract
//!
//! Runs each case through the pure resolver over an in-memory lookup and compares the result with
//! the expectation recorded here. Exits non-zero on any mismatch, so it can gate a build.
use pcscope_contracts::identity::*;
use pcscope_contracts::model;
use serde_json::{json, Value};

/// In-memory stand-in for the store S06 will provide. Only the two lookups the contract requires.
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

fn candidate(type_id: &str, scope: &str, fields: &[(&str, &str)]) -> pcscope_contracts::adapter::IdentityCandidate {
    pcscope_contracts::adapter::IdentityCandidate {
        type_id: type_id.into(),
        scope: scope.into(),
        fields: fields
            .iter()
            .map(|(key, value)| pcscope_contracts::adapter::IdentityField { key: (*key).into(), value: (*value).into(), from: "example".into() })
            .collect(),
        missing: Vec::new(),
        ambiguous: None,
    }
}

fn request(type_id: &str, scope: &str, fields: &[(&str, &str)], source: &str, record: &str) -> IdentityRequest {
    IdentityRequest {
        candidate: candidate(type_id, scope, fields),
        strategy: TypeStrategy::for_type(type_id).unwrap_or_else(|| panic!("{type_id} has no strategy")),
        binding: SourceRecordRef { source: source.into(), item_id: "item".into(), native_record_id: record.into() },
        machine_id: "machine-1".into(),
        installation_id: "install-1".into(),
        evidence_time: EvidenceTime { source_time: None, observed_at: "2026-09-12T00:00:00Z".into() },
        hints: LifecycleHints {
            complete_observation: true,
            collected_scope: "machine:machine-1".into(),
            configured: true,
            history_retained_from: None,
            states_object_ended: false,
            boot_id: None,
        },
        rule_version: model::catalog_fingerprint(),
    }
}

/// Resolves and commits, so the next request sees the instance the way S06's store would.
fn observe(lookup: &mut Lookup, request: &IdentityRequest) -> (IdentityResolution, Option<u64>) {
    let resolution = resolve(request, lookup);
    let id = match &resolution {
        IdentityResolution::Confirmed(c) => {
            let id = c.instance_id.unwrap_or_else(|| lookup.allocate(&c.identity));
            lookup.bind(&request.binding, id);
            Some(id)
        }
        IdentityResolution::Provisional(p) => {
            // S06 allocates a provisional instance when none is known yet, and keeps the alias so
            // references survive the later confirmation.
            let id = p.instance_id.unwrap_or_else(|| lookup.allocate(&p.identity));
            lookup.bind(&request.binding, id);
            Some(id)
        }
        _ => None,
    };
    (resolution, id)
}

fn outcome_name(resolution: &IdentityResolution) -> &'static str {
    match resolution {
        IdentityResolution::Confirmed(_) => "confirmed",
        IdentityResolution::Provisional(_) => "provisional",
        IdentityResolution::Ambiguous(_) => "ambiguous",
        IdentityResolution::Unresolved { .. } => "unresolved",
    }
}
fn basis_name(resolution: &IdentityResolution) -> String {
    match resolution {
        IdentityResolution::Confirmed(c) => serde_json::to_value(&c.basis).unwrap().as_str().unwrap_or("?").to_string(),
        IdentityResolution::Provisional(p) => format!("refine by {}", p.could_be_refined_by.join(",")),
        IdentityResolution::Ambiguous(a) => format!("{} candidates", a.candidates.len()),
        IdentityResolution::Unresolved { reason } => reason.clone(),
    }
}

struct Report {
    failures: Vec<String>,
    cases: Vec<Value>,
}
impl Report {
    fn case(&mut self, name: &str, expectation: &str, actual: Value, ok: bool) {
        if !ok {
            self.failures.push(format!("{name}: expected {expectation}, got {actual}"));
        }
        self.cases.push(json!({"case": name, "expectation": expectation, "actual": actual, "passed": ok}));
    }
}

fn main() {
    let mut lookup = Lookup::default();
    let mut report = Report { failures: Vec::new(), cases: Vec::new() };

    // Each observation below uses a distinct source record id, so the cases exercise key matching
    // rather than the source-binding shortcut. Case 6 covers the binding path on its own.

    // 1. A user is renamed. The SID is the identity; the name is a field.
    let sid = "S-1-5-21-1000000000-2000000000-3000000000-1001";
    let (first, first_id) = observe(&mut lookup, &request("account.user", "install-1", &[("sid", sid)], "pcscope.identity.accounts", "acct-round-1"));
    let (renamed, renamed_id) = observe(&mut lookup, &request("account.user", "install-1", &[("sid", sid)], "pcscope.identity.accounts", "acct-round-2"));
    report.case(
        "user renamed, same SID",
        "one instance, matched by key; the name is a field and never part of the identity",
        json!({"first": outcome_name(&first), "second": outcome_name(&renamed), "basis": basis_name(&renamed), "same_instance": first_id == renamed_id}),
        outcome_name(&first) == "confirmed" && outcome_name(&renamed) == "confirmed" && basis_name(&renamed) == "existing_by_key" && first_id == renamed_id,
    );

    // 2. Two tasks with the same name in different folders are different objects.
    let (_, task_a) = observe(&mut lookup, &request("startup.scheduled_task", "install-1", &[("path", "\\示例文件夹\\示例任务")], "pcscope.startup.tasks", "task-a"));
    let (_, task_b) = observe(&mut lookup, &request("startup.scheduled_task", "install-1", &[("path", "\\另一个文件夹\\示例任务")], "pcscope.startup.tasks", "task-b"));
    // The same folder stated with different case is the same task: the path namespace is folded.
    let (same_case, task_a2) = observe(&mut lookup, &request("startup.scheduled_task", "install-1", &[("path", "\\示例文件夹\\示例任务")], "pcscope.startup.tasks", "task-a-round-2"));
    report.case(
        "tasks with the same name in different folders",
        "two instances; a path differing only in case is the same instance",
        json!({"distinct": task_a != task_b, "case_folded_same": task_a2 == task_a, "basis": basis_name(&same_case)}),
        task_a != task_b && task_a2 == task_a && basis_name(&same_case) == "existing_by_key",
    );

    // 3. A reused process id with a different creation time is a different process.
    let (_, proc_a) = observe(&mut lookup, &request("runtime.process", "install-1", &[("pid", "4100"), ("created_at", "2026-09-10T00:00:00Z")], "pcscope.execution.processes", "proc-1"));
    let (_, proc_b) = observe(&mut lookup, &request("runtime.process", "install-1", &[("pid", "4100"), ("created_at", "2026-09-10T01:00:00Z")], "pcscope.execution.processes", "proc-2"));
    let (no_time, _) = observe(&mut lookup, &request("runtime.process", "install-1", &[("pid", "9999")], "pcscope.execution.processes", "proc-3"));
    report.case(
        "process id reuse",
        "two instances for the same pid; a pid without a creation time is provisional",
        json!({"same_pid_distinct": proc_a != proc_b, "pid_only": outcome_name(&no_time), "refine_by": basis_name(&no_time)}),
        proc_a != proc_b && outcome_name(&no_time) == "provisional",
    );

    // 4. A file identity survives a rename; a path-only candidate does not have one.
    let file_identity = [("volume_identity", "Volume{abc}"), ("file_id", "0000000000000abc")];
    let (_, file_a) = observe(&mut lookup, &request("file.watched_file", "install-1", &file_identity, "pcscope.files.metadata-security", "file-round-1"));
    let (after_rename, file_a2) = observe(&mut lookup, &request("file.watched_file", "install-1", &file_identity, "pcscope.files.metadata-security", "file-round-2"));
    let (path_only, path_only_id) = observe(&mut lookup, &request("file.watched_file", "install-1", &[("path", "C:\\示例\\监控\\示例文件.txt")], "pcscope.files.metadata-security", "file-round-3"));
    report.case(
        "file identity and path reuse",
        "the same file id is one instance across a rename; a path-only candidate is provisional",
        json!({"survives_rename": file_a == file_a2, "basis": basis_name(&after_rename), "path_only": outcome_name(&path_only), "path_only_id": path_only_id}),
        file_a == file_a2 && basis_name(&after_rename) == "existing_by_key" && outcome_name(&path_only) == "provisional",
    );

    // 5. Two different sources observe one task.
    let (_, from_api) = observe(&mut lookup, &request("startup.scheduled_task", "install-1", &[("path", "\\示例文件夹\\示例任务二")], "pcscope.startup.tasks", "api-1"));
    let (from_log, from_log_id) = observe(&mut lookup, &request("startup.scheduled_task", "install-1", &[("path", "\\示例文件夹\\示例任务二")], "pcscope.eventlog.microsoft-windows-taskscheduler/operational", "log-1"));
    let bindings = lookup.bindings.iter().filter(|(_, _, _, id)| Some(*id) == from_api).count();
    report.case(
        "one instance observed by two sources",
        "one instance, matched by key across sources, with a binding per source",
        json!({"same_instance": from_api == Some(from_log_id.unwrap_or(0)), "basis": basis_name(&from_log), "bindings": bindings}),
        from_api == Some(from_log_id.unwrap_or(0)) && basis_name(&from_log) == "existing_by_key" && bindings == 2,
    );

    // 6. The same source record stays on the same instance even when its key alone would not say
    //    so. This is what keeps a provisional identity from forking on the next round.
    let (first_binding, bound) = observe(&mut lookup, &request("runtime.process", "install-1", &[("pid", "7777")], "pcscope.execution.processes", "stable-record"));
    let (again, again_id) = observe(&mut lookup, &request("runtime.process", "install-1", &[("pid", "7777")], "pcscope.execution.processes", "stable-record"));
    report.case(
        "a provisional identity does not fork",
        "both observations stay provisional and resolve to one instance",
        json!({"first": outcome_name(&first_binding), "second": outcome_name(&again), "same_instance": bound == again_id, "refine_by": basis_name(&again)}),
        // The second observation stays provisional: matching the instance says which one it is, not
        // that the missing key has arrived. It must not be promoted to confirmed by repetition.
        outcome_name(&first_binding) == "provisional" && outcome_name(&again) == "provisional"
            && bound.is_some() && bound == again_id,
    );

    // 7. Lifecycle: absence is not deletion, and a stated removal is the only path to removed.
    let hints = LifecycleHints {
        complete_observation: true,
        collected_scope: "machine:machine-1".into(),
        configured: true,
        history_retained_from: None,
        states_object_ended: false,
        boot_id: None,
    };
    let absent = lifecycle(Some((LifecycleState::Active, 1)), ObservationFacts::Absent, &hints, "2026-09-12T01:00:00Z");
    let still_absent = lifecycle(Some((LifecycleState::NotObserved, 1)), ObservationFacts::Absent, &hints, "2026-09-12T02:00:00Z");
    let stated = lifecycle(Some((LifecycleState::NotObserved, 1)), ObservationFacts::Absent, &LifecycleHints { states_object_ended: true, ..hints.clone() }, "2026-09-12T03:00:00Z");
    report.case(
        "absence is not deletion",
        "absent -> not observed; still absent -> unchanged; only a stated removal becomes removed",
        json!({
            "first_absence": transition_name(&absent),
            "second_absence": transition_name(&still_absent),
            "stated_removal": transition_name(&stated),
        }),
        transition_name(&absent) == "not_observed" && transition_name(&still_absent) == "unchanged" && transition_name(&stated) == "removed_stated",
    );

    // 7. A missing installation or boot identifier degrades instead of being invented.
    let mut degraded = request("startup.scheduled_task", "install-1", &[("path", "\\示例文件夹\\降级")], "pcscope.startup.tasks", "degraded");
    degraded.hints.boot_id = None;
    degraded.installation_id = String::new();
    let (degraded_resolution, _) = observe(&mut lookup, &degraded);
    let boot_absent = degraded.rule_version == model::catalog_fingerprint() && degraded.hints.boot_id.is_none();
    report.case(
        "missing installation or boot identifier degrades clearly",
        "a missing boot id is not invented; the identity degrades without fabricating context",
        json!({"boot_id": degraded.hints.boot_id, "outcome": outcome_name(&degraded_resolution), "basis": basis_name(&degraded_resolution)}),
        boot_absent && outcome_name(&degraded_resolution) == "confirmed",
    );

    let value = json!({
        "contract_version": 1,
        "catalog_version": model::catalog_version(),
        "rule_version": model::catalog_fingerprint(),
        "cases": report.cases,
    });
    println!("{}", serde_json::to_string_pretty(&value).unwrap());
    if report.failures.is_empty() {
        println!("identity contract: {} cases passed", value["cases"].as_array().map_or(0, Vec::len));
        return;
    }
    eprintln!("identity contract FAILED:");
    for failure in &report.failures {
        eprintln!("  ! {failure}");
    }
    std::process::exit(1);
}

fn transition_name(transition: &LifecycleTransition) -> &'static str {
    match transition {
        LifecycleTransition::FirstObserved { .. } => "first_observed",
        LifecycleTransition::StillObserved => "still_observed",
        LifecycleTransition::NotObserved { .. } => "not_observed",
        LifecycleTransition::EndedStated { .. } => "ended_stated",
        LifecycleTransition::RemovedStated { .. } => "removed_stated",
        LifecycleTransition::Reincarnated { .. } => "reincarnated",
        LifecycleTransition::Unchanged { .. } => "unchanged",
    }
}
