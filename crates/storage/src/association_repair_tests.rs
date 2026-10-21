use super::*;
use crate::Store;
use pcscope_contracts::parse;

fn ingest(store:&mut Store,id:usize,path:Value,provider:&str) {
    store.ingest(&parse(&json!({"protocol_version":1,"request_id":format!("{id}"),"type":"event_batch",
        "source":"pcscope.eventlog.microsoft-windows-taskscheduler/operational","epoch":"epoch-one","batch_seq":id,"checkpoint":format!("{id}"),
        "events":[{"id":format!("{id}"),"object_id":format!("{id}"),"kind":"startup.task_event","occurred_at":format!("2026-10-12T08:{id:02}:00Z"),
          "data":{"event_id":102,"provider":provider,"fields":{"TaskName":path,"InstanceId":format!("run-{id}")}}}]}).to_string()).unwrap()).unwrap();
    while store.project_pending("M","I",64).unwrap().examined>0 {}
}
fn task(store:&mut Store,id:usize,path:Value){ingest(store,id,path,"Microsoft-Windows-TaskScheduler")}
fn repair(store:&Store,limit:usize)->Value {
    for _ in 0..100 {let value=store.repair_event_associations(limit,Duration::from_secs(1)).unwrap();if value["pending"]==false{return value}}
    panic!("repair did not finish")
}

#[test]
fn native_task_end_paths_are_separate_and_missing_paths_do_not_share_an_object() {
    let mut store=Store::open(":memory:").unwrap();
    task(&mut store,1,json!("\\Folder\\One"));task(&mut store,2,json!("\\Folder\\Two"));
    task(&mut store,3,Value::Null);task(&mut store,4,json!({"protected":true}));
    let n:i64=store.reader().query_row("SELECT count(DISTINCT instance_id) FROM object_actions WHERE action_id='startup.task_ended'",[],|r|r.get(0)).unwrap();assert_eq!(n,2);
    let n:i64=store.reader().query_row("SELECT count(*) FROM object_actions WHERE instance_id IS NULL",[],|r|r.get(0)).unwrap();assert_eq!(n,2);
    ingest(&mut store,5,json!("\\Wrong"),"OtherProvider");
    assert_eq!(store.reader().query_row("SELECT count(*) FROM object_actions",[],|r|r.get::<_,i64>(0)).unwrap(),4);
}

#[test]
fn split_shared_task_history_preserves_actions_current_state_and_replay_identity() {
    let mut store=Store::open(":memory:").unwrap();task(&mut store,1,json!("\\Folder\\One"));task(&mut store,2,json!("\\Folder\\Two"));
    let conn=store.reader();
    conn.execute_batch("INSERT INTO object_instances(type_id,category,machine_id,installation_id,scope,native_key,incarnation,identity_status,lifecycle,first_observed_at,last_observed_at)
       VALUES('startup.scheduled_task','startup','M','I','machine:M','machine:M|?path',1,'provisional','active','2026-10-01T00:00:00Z','2026-10-21T00:00:00Z');").unwrap();
    let legacy=conn.last_insert_rowid();
    conn.execute("UPDATE object_actions SET instance_id=?",[legacy]).unwrap();
    conn.execute("UPDATE source_bindings SET instance_id=?",[legacy]).unwrap();
    let before:Vec<(i64,String,String)>=conn.prepare("SELECT action_row,occurred_at,observed_at FROM object_actions ORDER BY action_row").unwrap().query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap().collect::<rusqlite::Result<_>>().unwrap();
    let current:Vec<(i64,String,String)>=conn.prepare("SELECT instance_id,lifecycle,last_observed_at FROM object_instances ORDER BY instance_id").unwrap().query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap().collect::<rusqlite::Result<_>>().unwrap();
    let result=repair(&store,1);assert_eq!(result["repaired"],2);assert_eq!(result["unresolved"],0);
    let conn=store.reader();
    let after:Vec<(i64,String,String)>=conn.prepare("SELECT action_row,occurred_at,observed_at FROM object_actions ORDER BY action_row").unwrap().query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap().collect::<rusqlite::Result<_>>().unwrap();assert_eq!(before,after);
    let state:Vec<(i64,String,String)>=conn.prepare("SELECT instance_id,lifecycle,last_observed_at FROM object_instances ORDER BY instance_id").unwrap().query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap().collect::<rusqlite::Result<_>>().unwrap();assert_eq!(current,state);
    assert_eq!(conn.query_row("SELECT count(DISTINCT instance_id) FROM object_actions",[],|r|r.get::<_,i64>(0)).unwrap(),2);
    assert_eq!(conn.query_row("SELECT count(*) FROM source_bindings WHERE instance_id=? AND valid_to IS NULL",[legacy],|r|r.get::<_,i64>(0)).unwrap(),0);
    assert_eq!(repair(&store,1)["repaired"],2);
    // Replaying all retained raw events after correction must not create more actions.
    conn.execute("UPDATE projection_queue SET state='pending'",[]).unwrap();
    while store.project_pending("M","I",64).unwrap().examined>0 {}
    assert_eq!(conn.query_row("SELECT count(*) FROM object_actions",[],|r|r.get::<_,i64>(0)).unwrap(),2);
    assert_eq!(conn.query_row("SELECT count(*) FROM records",[],|r|r.get::<_,i64>(0)).unwrap(),2);
}

#[test]
fn conflicting_task_evidence_does_not_reassign_a_shared_action() {
    let mut store=Store::open(":memory:").unwrap();task(&mut store,1,json!("\\One"));task(&mut store,2,json!("\\Two"));
    let c=store.reader();c.execute_batch("INSERT OR IGNORE INTO evidence_refs SELECT 'action','1',evidence_kind,evidence_id,pruned_at FROM evidence_refs WHERE subject_kind='action' AND subject_id='2';").unwrap();
    let original:i64=c.query_row("SELECT instance_id FROM object_actions WHERE action_row=1",[],|r|r.get(0)).unwrap();
    assert_eq!(repair(&store,2)["unresolved"],1);
    assert_eq!(c.query_row("SELECT instance_id FROM object_actions WHERE action_row=1",[],|r|r.get::<_,i64>(0)).unwrap(),original);
    assert!(store.repair_event_associations(0,Duration::ZERO).is_err());
}

#[test]
fn repair_rolls_back_atomically_and_resumes_after_reopen() {
    let path=std::env::temp_dir().join(format!("pcscope-association-{}-{}.db",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    {
        let mut store=Store::open(&path).unwrap();task(&mut store,1,json!("\\One"));task(&mut store,2,json!("\\Two"));
        let c=store.reader();c.execute_batch("UPDATE object_instances SET native_key='machine:M|?path',identity_status='provisional' WHERE instance_id=1;
          UPDATE object_actions SET instance_id=1; UPDATE source_bindings SET instance_id=1;
          CREATE TRIGGER fail_repair BEFORE UPDATE OF instance_id ON object_actions BEGIN SELECT RAISE(ABORT,'injected repair failure'); END;").unwrap();
        assert!(store.repair_event_associations(1,Duration::from_secs(1)).is_err());
        assert_eq!(c.query_row("SELECT count(*) FROM association_repair_audit",[],|r|r.get::<_,i64>(0)).unwrap(),0);
        assert_eq!(c.query_row("SELECT count(*) FROM association_repair_state",[],|r|r.get::<_,i64>(0)).unwrap(),0);
        assert_eq!(c.query_row("SELECT count(*) FROM object_instances",[],|r|r.get::<_,i64>(0)).unwrap(),2);
        c.execute_batch("DROP TRIGGER fail_repair").unwrap();
        let first=store.repair_event_associations(1,Duration::from_secs(1)).unwrap();assert_eq!(first["repaired"],1);assert_eq!(first["pending"],true);
    }
    {
        let store=Store::open(&path).unwrap();let result=repair(&store,1);assert_eq!(result["repaired"],2);assert_eq!(result["unresolved"],0);
        assert_eq!(store.reader().query_row("SELECT count(*) FROM association_repair_audit WHERE outcome='repaired'",[],|r|r.get::<_,i64>(0)).unwrap(),2);
    }
    std::fs::remove_file(path).unwrap();
}
