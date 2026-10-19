use super::*;

fn progress(store:&Store)->String {
    store.reader().query_row("SELECT json_array(revision,object_cursor,action_cursor,evidence_cursor,generation,published) FROM display_build WHERE id=1",[],|r|r.get(0)).unwrap()
}

#[test]
fn online_read_indexes_preserve_history_and_progress_and_existing_writer() {
    let path=std::env::temp_dir().join(format!("pcscope-online-index-{}-{}.db",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let mut store=Store::open_program(&path).unwrap();
    let fixture=|run:&str,enabled:bool|pcscope_contracts::parse(&json!({"protocol_version":1,"request_id":run,"type":"snapshot","source":"pcscope.startup.tasks","scope":"machine:M/schema:1","run_id":run,"observed_at":format!("2026-10-19T0{}:00:00Z",if enabled {1}else{2}),"complete":true,"items":[{"id":"\\Task","data":{"path":"\\Task","name":"Task","enabled":enabled}}]}).to_string()).unwrap();
    store.ingest(&fixture("one",true)).unwrap();store.project_pending("M","I",100).unwrap();
    store.ingest(&fixture("two",false)).unwrap();store.project_pending("M","I",100).unwrap();
    store.prepare_display(32,std::time::Duration::from_secs(1)).unwrap();
    store.conn.execute_batch("DROP INDEX projection_queue_pending; DROP INDEX evidence_refs_action_address;").unwrap();
    let before=progress(&store);
    let raw:String=store.conn.query_row("SELECT group_concat(payload) FROM snapshots",[],|r|r.get(0)).unwrap();
    for _ in 0..2 {assert_eq!(Store::install_read_indexes(&path).unwrap()["indexes_ready"],true);}
    assert_eq!(progress(&store),before,"online maintenance must not restart the display rebuild");
    assert_eq!(store.conn.query_row("SELECT group_concat(payload) FROM snapshots",[],|r|r.get::<_,String>(0)).unwrap(),raw);
    assert_eq!(store.conn.query_row("PRAGMA user_version",[],|r|r.get::<_,i64>(0)).unwrap(),10);
    let plan=|sql:&str|store.conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap().query_map([],|r|r.get::<_,String>(3)).unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap().join("\n");
    assert!(plan("SELECT count(*) FROM projection_queue WHERE state='pending'").contains("projection_queue_pending"));
    assert!(plan("SELECT 1 FROM object_actions a JOIN evidence_refs r ON r.subject_kind='action' AND r.subject_id=CAST(a.action_row AS TEXT) WHERE a.instance_id=1 AND r.evidence_id='missing'").contains("evidence_refs_action_address"));
    // The already-open writer need not know about the added index to keep it correct.
    store.ingest(&fixture("three",true)).unwrap();
    assert!(store.conn.query_row("SELECT count(*) FROM projection_queue INDEXED BY projection_queue_pending WHERE state='pending'",[],|r|r.get::<_,i64>(0)).unwrap()>0);
    store.project_pending("M","I",100).unwrap();
    assert_eq!(store.conn.query_row("SELECT count(*) FROM projection_queue INDEXED BY projection_queue_pending WHERE state='pending'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    store.conn.execute_batch("DROP INDEX projection_queue_pending; UPDATE settings SET value='foreign' WHERE key='native_program_identity';").unwrap();
    assert!(Store::install_read_indexes(&path).is_err());
    assert_eq!(store.conn.query_row("SELECT count(*) FROM sqlite_schema WHERE name='projection_queue_pending'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    drop(store);std::fs::remove_file(path).unwrap();
}
