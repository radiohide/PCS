//! S29 fault injection.
//!
//! The scenarios the plan names, run against the real store rather than described: a transaction
//! that fails, a process that stops, work that is retried, a source log that resets, a source that
//! fails while others succeed, evidence that is pruned, and a reader working while a writer does.
//!
//! Two rules the tests hold to:
//!
//!   * **Nothing is forced by damaging the real machine.** Every fault is injected into a database
//!     this module created, in a temporary directory. The plan forbids producing samples by clearing
//!     a system log, and that prohibition is really about a wider habit: a test that needs the host
//!     to be in a particular state is not a test anybody can repeat.
//!   * **A fault is checked by what survived it.** A failed transaction is judged by the rows that
//!     did not appear, a stopped process by what a later run finds, a retry by what it did not
//!     duplicate.

#[cfg(test)]
mod tests {
    use crate::Store;
    use serde_json::json;

/// A store in its own temporary file, for the faults that need a real file rather than `:memory:`
/// — a second connection, or a process that stops without running a shutdown.
pub(crate) fn temporary_store(name: &str) -> (Store, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!("pcscope-fault-{}-{}-{}.db", name, std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)));
    let _ = std::fs::remove_file(&path);
    (Store::open(&path).expect("a temporary store opens"), path)
}

pub(crate) fn cleanup(path: &std::path::Path) {
    for suffix in ["", "-wal", "-shm"] {
        let mut name = path.as_os_str().to_os_string();
        name.push(suffix);
        let _ = std::fs::remove_file(std::path::PathBuf::from(name));
    }
}


    const TASKS: &str = "pcscope.startup.tasks";
    const SCOPE: &str = "machine:M/install:I/schema:1/context:/shard:0";

    fn snapshot(run: &str, names: &[&str]) -> pcscope_contracts::Envelope {
        let items: Vec<serde_json::Value> = names.iter().map(|name| json!({"id": format!("\\A\\{name}"), "data": {"path": format!("\\A\\{name}"), "name": name}})).collect();
        pcscope_contracts::parse(&json!({
            "protocol_version": 1, "request_id": run, "type": "snapshot", "source": TASKS,
            "scope": SCOPE, "run_id": run, "observed_at": "2026-09-12T00:00:00Z", "complete": true, "items": items
        }).to_string())
        .unwrap()
    }

    fn count(store: &Store, sql: &str) -> i64 {
        store.conn.query_row(sql, [], |r| r.get(0)).unwrap()
    }

    /// A batch that fails leaves nothing at all: no evidence, no work item, no advanced checkpoint.
    /// This is the fault the whole design turns on — a source that re-sends after a failure must
    /// find the store exactly as it was.
    #[test]
    fn a_failed_batch_leaves_no_trace() {
        let mut store = Store::open(":memory:").unwrap();
        store.ingest(&snapshot("run-1", &["task-00"])).unwrap();
        let before = (count(&store, "SELECT count(*) FROM evidence"), count(&store, "SELECT count(*) FROM snapshots"), count(&store, "SELECT count(*) FROM projection_queue"));
        // A run id already taken with different content is refused, which rolls the whole batch back.
        let mut conflicting = snapshot("run-1", &["task-99"]);
        if let pcscope_contracts::Message::Snapshot { items, .. } = &mut conflicting.message {
            items[0].data = json!({"path": "\\A\\changed"});
        }
        assert!(store.ingest(&conflicting).is_err(), "a reused run id with different content must be refused");
        assert_eq!(
            (count(&store, "SELECT count(*) FROM evidence"), count(&store, "SELECT count(*) FROM snapshots"), count(&store, "SELECT count(*) FROM projection_queue")),
            before,
            "a failed batch must not leave evidence, material or work behind"
        );
    }

    /// A process that stops mid-transaction leaves the database as it was: the next run sees the
    /// last committed state, not a half-written one.
    #[test]
    fn an_abandoned_transaction_leaves_the_database_as_it_was() {
        let (store, path) = temporary_store("abandoned");
        let mut writer = Store::open(&path).unwrap();
        writer.ingest(&snapshot("run-1", &["task-00"])).unwrap();
        let committed = count(&writer, "SELECT count(*) FROM evidence");

        // A transaction that is begun and never committed, dropped as a process death would drop it.
        {
            let tx = writer.conn.unchecked_transaction().unwrap();
            tx.execute("INSERT INTO evidence(evidence_id,item_id,kind,source,structure_version,source_scope,native_record_id,observed_at,ingested_at,decoding,payload_ref) VALUES('ghost','ghost','snapshot','ghost','v1','s','r','2026-09-12T00:00:00Z','2026-09-12T00:00:00Z','{}','nowhere')", []).unwrap();
            drop(tx);
        }
        // A second store, opened fresh, sees the committed state and no ghost.
        let reader = Store::open(&path).unwrap();
        assert_eq!(count(&reader, "SELECT count(*) FROM evidence"), committed);
        assert_eq!(count(&reader, "SELECT count(*) FROM evidence WHERE evidence_id='ghost'"), 0);
        drop(store);
        cleanup(&path);
    }

    /// A reader works while a writer is mid-transaction, and sees the committed state rather than a
    /// partial one. This is what the independent WAL reader exists for.
    #[test]
    fn a_reader_works_while_a_writer_is_running() {
        let (store, path) = temporary_store("concurrent");
        let mut writer = Store::open(&path).unwrap();
        writer.ingest(&snapshot("run-1", &["task-00"])).unwrap();
        writer.project_pending("M", "I", 100).unwrap();
        let settled = count(&writer, "SELECT count(*) FROM object_instances");

        // A query through the read path while the writer holds a transaction open.
        let tx = writer.conn.unchecked_transaction().unwrap();
        tx.execute("INSERT INTO retention_units(created_at) VALUES('2026-09-12T00:00:00Z')", []).unwrap();
        let read = Store::open_reader(&path, std::time::Duration::from_secs(5)).unwrap();
        let observed: i64 = read.conn.query_row("SELECT count(*) FROM object_instances", [], |r| r.get(0)).unwrap();
        assert_eq!(observed, settled, "a reader must see the last committed state, not the open transaction");
        drop(tx);
        drop(read);
        drop(store);
        cleanup(&path);
    }

    /// A source that fails does not stop the others, and its failure is stated rather than silent.
    #[test]
    fn one_failing_source_does_not_stop_the_others() {
        let mut store = Store::open(":memory:").unwrap();
        store.ingest(&snapshot("run-1", &["task-00"])).unwrap();
        let unknown = pcscope_contracts::parse(&json!({
            "protocol_version": 1, "request_id": "run-x", "type": "snapshot", "source": "pcscope.unconverted.thing",
            "scope": SCOPE, "run_id": "run-x", "observed_at": "2026-09-12T00:00:00Z", "complete": true,
            "items": [{"id": "x", "data": {}}]
        }).to_string())
        .unwrap();
        store.ingest(&unknown).unwrap();
        store.ingest(&snapshot("run-2", &["task-01"])).unwrap();

        let report = store.project_pending("M", "I", 100).unwrap();
        assert_eq!(report.projected, 3, "both readable observations and the added task reach projection");
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE action_id='observed.first_seen'"),1,"the later task is new, rather than part of the initial baseline");
        assert_eq!(report.unadapted, 1, "the unreadable one is reported, not dropped");
        assert_eq!(count(&store, "SELECT count(distinct source) FROM evidence"), 2, "the source whose material failed is still retained");
        assert_eq!(count(&store, "SELECT count(*) FROM evidence WHERE source='pcscope.unconverted.thing'"), 1);
    }

    /// Work that failed and is run again duplicates nothing.
    #[test]
    fn retrying_failed_work_does_not_duplicate_anything() {
        let mut store = Store::open(":memory:").unwrap();
        store.ingest(&snapshot("run-1", &["task-00", "task-01"])).unwrap();
        store.project_pending("M", "I", 100).unwrap();
        let settled = (count(&store, "SELECT count(*) FROM object_facts"), count(&store, "SELECT count(*) FROM object_actions"));

        // The queue is put back as a retry would: the work item outstanding again.
        store.conn.execute("UPDATE projection_queue SET state='pending'", []).unwrap();
        store.project_pending("M", "I", 100).unwrap();
        assert_eq!(
            (count(&store, "SELECT count(*) FROM object_facts"), count(&store, "SELECT count(*) FROM object_actions")),
            settled,
            "a retried batch must not produce a second copy of its facts or actions"
        );
    }

    /// Pruned evidence keeps the conclusion and marks the reference, rather than leaving a pointer
    /// into a row that is gone.
    #[test]
    fn pruned_evidence_keeps_the_conclusion_and_marks_the_reference() {
        let mut store = Store::open(":memory:").unwrap();
        store.ingest(&snapshot("run-1", &["task-00"])).unwrap();
        store.project_pending("M", "I", 100).unwrap();
        let value: String = store.conn.query_row("SELECT value FROM object_facts WHERE field_id='name'", [], |r| r.get(0)).unwrap();

        let unit: i64 = store.conn.query_row("SELECT unit_id FROM snapshots ORDER BY rowid LIMIT 1", [], |r| r.get(0)).unwrap();
        let tx = store.conn.unchecked_transaction().unwrap();
        crate::model::mark_pruned_snapshots(&tx, unit, "2026-09-12T02:00:00Z").unwrap();
        crate::evidence::mark_pruned_for_unit(&tx, unit, "2026-09-12T02:00:00Z").unwrap();
        tx.execute("DELETE FROM snapshots WHERE unit_id=?", [unit]).unwrap();
        tx.commit().unwrap();

        assert_eq!(store.conn.query_row("SELECT value FROM object_facts WHERE field_id='name'", [], |r| r.get::<_, String>(0)).unwrap(), value, "the conclusion survives its evidence");
        assert!(count(&store, "SELECT count(*) FROM evidence_refs WHERE pruned_at IS NULL") == 0, "every reference to the pruned material says so");
        assert_eq!(count(&store, "SELECT count(*) FROM evidence_refs"), count(&store, "SELECT count(*) FROM evidence_refs WHERE pruned_at IS NOT NULL"));
    }

    /// The structural invariants hold after a full run.
    #[test]
    fn the_structural_invariants_hold_after_a_full_run() {
        let mut store = Store::open(":memory:").unwrap();
        store.ingest(&snapshot("run-1", &["task-00", "task-01"])).unwrap();
        store.ingest(&snapshot("run-2", &["task-00", "task-02"])).unwrap();
        store.project_pending("M", "I", 100).unwrap();

        // Every reference points at something that exists, and every foreign key is satisfied. The
        // plan asks for this check specifically, and it is the one that catches a cascade that was
        // never written or a delete that left rows behind.
        let missing: i64 = store.conn.query_row(
            "SELECT count(*) FROM evidence_refs r WHERE r.pruned_at IS NULL AND r.subject_kind='observation'
               AND NOT EXISTS (SELECT 1 FROM evidence e WHERE e.evidence_id=r.evidence_id)",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(missing, 0, "no reference points into nothing");
        let violations: i64 = store.conn.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r.get(0)).unwrap();
        assert_eq!(violations, 0, "every foreign key is satisfied");
        let report = store.reconcile().unwrap();
        assert_eq!(crate::reconcile::verdict(&report), "reconciled");
    }
}
