//! S31 user-flow regression: plugin configuration through to evidence.
//!
//! One walk of the whole chain, asserted at every stage rather than at the end:
//!
//!   configuration → collection material → evidence → identity → facts and actions → query → evidence
//!
//! The point of walking it in one place is that each stage is checked against what the next one
//! needs. A report that only asserts the last step passes whenever an earlier one silently produced
//! nothing, and the loop is exactly the shape a refactor breaks: every stage can look healthy on
//! its own while the chain between them carries nothing.
//!
//! What this is not: the real desktop connecting to a real daemon, a real collector emitting to it,
//! or a machine where permissions are actually missing. Those need hardware this step does not have
//! and are recorded as such.

#[cfg(test)]
mod tests {
    use crate::Store;
    use serde_json::json;

    const TASKS: &str = "pcscope.startup.tasks";
    const TASK_DETAILS: &str = "pcscope.startup.task-details";
    const RUN_SOURCE: &str = "pcscope.eventlog.microsoft-windows-taskscheduler/operational";
    const TASK_PATH: &str = r"\A\task";
    /// The shape a collector actually emits: the machine and the collecting account, then how the
    /// collection was cut up. There is no install segment, which is why the adapters narrow to the
    /// installation themselves.
    const SCOPE: &str = "machine:M/user:S-1-5-18/schema:1/context:/shard:0";

    fn snapshot(source: &str, run: &str, observed_at: &str, name: &str) -> pcscope_contracts::Envelope {
        pcscope_contracts::parse(&json!({
            "protocol_version": 1, "request_id": run, "type": "snapshot", "source": source,
            "scope": SCOPE, "run_id": run, "observed_at": observed_at, "complete": true,
            "items": [{"id": TASK_PATH, "data": {"path": TASK_PATH, "name": name, "enabled": true}}]
        }).to_string())
        .unwrap()
    }

    /// The shape a collector actually sends: the action id in `kind`, and the projected fields
    /// under their Windows names. A fixture-shaped event would exercise the fallback path instead,
    /// and the fallback has no way to name a task by its path.
    fn run_log() -> pcscope_contracts::Envelope {
        pcscope_contracts::parse(&json!({
            "protocol_version": 1, "request_id": "batch-1", "type": "event_batch", "source": RUN_SOURCE,
            "epoch": "1", "batch_seq": 1, "checkpoint": "cp-1",
            "events": [
                {"id": "100", "occurred_at": "2026-09-12T08:00:00Z", "kind": "startup.task_started", "object_id": "100",
                 "data": {"provider": "Microsoft-Windows-TaskScheduler", "event_id": 100, "fields": {"TaskName": TASK_PATH}}},
                {"id": "102", "occurred_at": "2026-09-12T08:00:05Z", "kind": "startup.task_ended", "object_id": "102",
                 "data": {"provider": "Microsoft-Windows-TaskScheduler", "event_id": 102, "fields": {"TaskName": TASK_PATH}}}
            ]
        }).to_string())
        .unwrap()
    }

    fn count(store: &Store, sql: &str) -> i64 {
        store.reader().query_row(sql, [], |r| r.get(0)).unwrap()
    }

    fn action_ids(store: &Store) -> Vec<String> {
        let mut statement = store.reader().prepare("SELECT action_id FROM object_actions ORDER BY action_row").unwrap();
        let rows = statement.query_map([], |row| row.get::<_, String>(0)).unwrap();
        rows.collect::<rusqlite::Result<Vec<_>>>().unwrap()
    }

    /// The walk, asserted stage by stage.
    #[test]
    fn the_whole_chain_carries_one_task_from_a_collector_to_its_evidence() {
        let mut store = Store::open(":memory:").unwrap();

        // ---- collection material reaches the store: two configuration sources and one run log
        store.ingest(&snapshot(TASKS, "run-1", "2026-09-12T01:00:00Z", "示例任务")).unwrap();
        store.ingest(&snapshot(TASK_DETAILS, "run-2", "2026-09-12T01:00:00Z", "示例任务")).unwrap();
        store.ingest(&run_log()).unwrap();

        // ---- evidence exists for every message, with a work item each
        assert_eq!(count(&store, "SELECT count(*) FROM evidence"), 4, "one address per snapshot item and per event");
        assert_eq!(count(&store, "SELECT count(*) FROM projection_queue WHERE state='pending'"), 4);

        // ---- projection: two sources and a run log describe one task
        let report = store.project_pending("M", "I", 100).unwrap();
        assert_eq!(report.projected, 4);
        assert_eq!(count(&store, "SELECT count(*) FROM object_instances"), 1);
        assert_eq!(count(&store, "SELECT count(*) FROM projection_queue WHERE state='failed'"), 0);

        // ---- facts: the same field from both configuration sources cites both
        let cited: i64 = count(&store, "SELECT count(*) FROM evidence_refs r JOIN evidence e ON e.evidence_id=r.evidence_id WHERE r.subject_kind='observation' AND e.source='pcscope.startup.tasks'");
        assert!(cited > 0, "the field cites the source it came from");

        // ---- actions: the run log's start and end are two actions, not one success
        assert_eq!(action_ids(&store), vec!["startup.task_started".to_owned(), "startup.task_ended".to_owned()]);

        // ---- configuration: a collection item switched off after it has been collected, then on
        // again. The interval it spent off is not observed, and the coverage says so. Switching an
        // item off before anything was ever collected leaves no such interval, which is why this
        // comes after the first round rather than before it.
        store.set_item_enabled("pcscope-powershell-adapter.exe", "pcscope.startup.tasks", false, "2026-09-12T02:00:00Z").unwrap();
        assert!(store.item_controls().unwrap()["items"].as_array().unwrap().iter().any(|item| item["id"] == "pcscope.startup.tasks" && item["enabled"] == false));
        // Each change is itself evidence: PCScope's own activity is recorded like any other
        // material, so a reader can see when the collector was switched off and on again.
        assert_eq!(count(&store, "SELECT count(*) FROM evidence WHERE source='pcscope.core'"), 1, "the switch off is recorded");
        store.set_item_enabled("pcscope-powershell-adapter.exe", "pcscope.startup.tasks", true, "2026-09-12T02:00:01Z").unwrap();
        store.ingest(&snapshot(TASKS, "run-3", "2026-09-12T03:00:00Z", "示例任务")).unwrap();

        // ---- the application's own activity is not an object, and saying so is not a failure:
        // counting it as one would put errors on the health page for the application describing
        // itself. Everything else is projected.
        let report = store.project_pending("M", "I", 100).unwrap();
        assert_eq!(report.projected, 1, "the second round's snapshot");
        assert_eq!(report.skipped, 2, "two configuration changes settle as skipped");
        assert_eq!(count(&store, "SELECT count(*) FROM projection_queue WHERE state='failed'"), 0);
        assert_eq!(count(&store, "SELECT count(*) FROM object_instances"), 1, "still one task");

        // ---- query: the object, its actions and its evidence are all reachable through the contract
        let list = store.query(&pcscope_contracts::query::QueryRequest::InstanceList(pcscope_contracts::query::InstanceFilter::default())).unwrap();
        assert_eq!(list.status, pcscope_contracts::query::ResponseStatus::Loaded);
        let row = &list.data.as_ref().unwrap()[0];
        let alias = row["alias"].as_str().unwrap().to_owned();
        let detail = store.query(&pcscope_contracts::query::QueryRequest::InstanceDetail { instance: alias }).unwrap();
        assert_eq!(detail.data.as_ref().unwrap()["summary"]["display_name"], "示例任务");

        let timeline = store.query(&pcscope_contracts::query::QueryRequest::ActionTimeline(pcscope_contracts::query::ActionFilter::default())).unwrap();
        let first = &timeline.data.as_ref().unwrap()[0];
        assert_eq!(first["instance_id"].as_i64(), Some(1), "the timeline's actions belong to the object");

        // ---- evidence: the address the record carries resolves to material that is still there
        let address: String = store.reader().query_row("SELECT evidence_id FROM evidence WHERE source=? ORDER BY seq LIMIT 1", [TASKS], |r| r.get(0)).unwrap();
        let evidence_view = store
            .query(&pcscope_contracts::query::QueryRequest::EvidenceDetail { evidence: pcscope_contracts::adapter::EvidenceRef { kind: pcscope_contracts::adapter::EvidenceKind::Snapshot, id: address } })
            .unwrap();
        assert_eq!(evidence_view.data.as_ref().unwrap()["raw_available"], true);

        // ---- coverage: the interval the item spent switched off is a gap, not a silent caught-up
        let coverage = store.coverage_report().unwrap();
        let rows = coverage.as_array().unwrap();
        // The fallback is a contract word, not an invented one: a source with no row is a source
        // nothing has been established about, which is `unknown`. It never fires here — the row is
        // always present — but a bare `"missing"` would have been a seventh word in a six-word
        // vocabulary the moment it did.
        let task_history = rows.iter().find(|row| row["source"] == TASKS).and_then(|row| row["history_state"].as_str()).unwrap_or("unknown");
        assert_eq!(task_history, "gap", "the interval the item spent switched off is not covered: {coverage}");

        // ---- and the whole thing reconciles
        assert_eq!(crate::reconcile::verdict(&store.reconcile().unwrap()), "reconciled");
    }

    /// A flow that is run twice adds nothing: the user doing the same thing again is not a change.
    #[test]
    fn running_the_same_flow_again_changes_nothing() {
        let mut store = Store::open(":memory:").unwrap();
        let envelope = snapshot(TASKS, "run-1", "2026-09-12T01:00:00Z", "示例任务");
        store.ingest(&envelope).unwrap();
        store.project_pending("M", "I", 100).unwrap();
        let settled = (count(&store, "SELECT count(*) FROM object_instances"), count(&store, "SELECT count(*) FROM object_facts"), action_ids(&store).len());

        // The collector re-sends the same run, as a retry after a lost acknowledgement would.
        store.ingest(&envelope).unwrap();
        store.project_pending("M", "I", 100).unwrap();
        assert_eq!(
            (count(&store, "SELECT count(*) FROM object_instances"), count(&store, "SELECT count(*) FROM object_facts"), action_ids(&store).len()),
            settled
        );
    }
}
