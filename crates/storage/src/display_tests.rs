use super::*;
use crate::Store;
use pcscope_contracts::query::{ActionFilter,InstanceFilter,QueryRequest};

fn snapshot(store:&mut Store,run:&str,at:&str,name:&str,enabled:bool,scope:&str) {
    store.ingest(&pcscope_contracts::parse(&json!({"protocol_version":1,"request_id":run,"type":"snapshot",
        "source":"pcscope.startup.tasks","scope":scope,"run_id":run,"observed_at":at,"complete":true,
        "items":[{"id":"\\Demo","data":{"path":"\\Demo","name":name,"enabled":enabled}}]}).to_string()).unwrap()).unwrap();
    store.project_pending("M","I",100).unwrap();
}
const SCOPE:&str="machine:M/schema:1/context:/shard:0";
fn material()->Store {
    let mut store=Store::open(":memory:").unwrap();
    snapshot(&mut store,"first","2026-10-16T01:00:00Z","原来的名称",true,SCOPE);
    snapshot(&mut store,"second","2026-10-16T02:00:00Z","原来的名称",false,SCOPE);
    store
}
fn finish(store:&Store) {
    for _ in 0..300 {
        let report=store.prepare_display(32,Duration::from_millis(50)).unwrap();
        assert_eq!(report["failed"],0,"{report}");
        if report["pending"]==false{return;}
    }
    panic!("display maintenance failed to converge: {}",status(store.reader()).unwrap());
}
fn list(store:&Store)->Value {store.query(&QueryRequest::InstanceList(InstanceFilter::default())).unwrap().data.unwrap()}
fn timeline(store:&Store)->Value {store.query(&QueryRequest::ActionTimeline(ActionFilter{include_routine:true,..Default::default()})).unwrap().data.unwrap()}

#[test]
fn stored_views_match_queries_reads_do_not_build_and_timestamp_polls_do_not_invalidate() {
    let mut store=material();let before=list(&store);let actions=timeline(&store);
    let pending:i64=store.reader().query_row("SELECT count(*) FROM display_work",[],|r|r.get(0)).unwrap();
    assert_eq!(store.reader().query_row("SELECT count(*) FROM display_objects",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    assert_eq!(pending,store.reader().query_row("SELECT count(*) FROM display_work",[],|r|r.get::<_,i64>(0)).unwrap(),"queries do not enqueue work");
    finish(&store);assert_eq!(before,list(&store));assert_eq!(actions,timeline(&store));
    assert!(cached(store.reader(),"object",1).unwrap().is_some());
    snapshot(&mut store,"unchanged","2026-10-16T03:00:00Z","原来的名称",false,SCOPE);
    assert!(cached(store.reader(),"object",1).unwrap().is_some(),"ordinary poll timestamps do not rebuild display facts");
    assert_eq!(list(&store)[0]["seen_at"],"2026-10-16T03:00:00Z");
    let latest:String=store.reader().query_row("SELECT last_attempt_at FROM coverage_scopes WHERE scope=?",[SCOPE],|r|r.get(0)).unwrap();
    assert_eq!(list(&store)[0]["coverage"][0]["last_attempt_at"],latest);
}

#[test]
fn changes_invalidate_transactionally_and_rollbacks_preserve_cached_values() {
    let store=material();finish(&store);let before=list(&store);
    {
        let tx=store.reader().unchecked_transaction().unwrap();
        tx.execute("UPDATE object_facts SET value='\"新名称\"' WHERE field_id='name'",[]).unwrap();
        assert!(cached(&tx,"object",1).unwrap().is_none());
    }
    assert_eq!(before,list(&store));assert!(cached(store.reader(),"object",1).unwrap().is_some());
    store.reader().execute("UPDATE object_facts SET value='\"新名称\"' WHERE field_id='name'",[]).unwrap();
    finish(&store);assert_eq!(list(&store)[0]["presentation"]["name"],"新名称");
}

#[test]
fn identity_changes_immediately_reject_old_context_and_rebuild_history_in_slices() {
    let store=material();finish(&store);
    let action:i64=store.reader().query_row("SELECT min(action_row) FROM object_actions",[],|r|r.get(0)).unwrap();
    assert!(cached(store.reader(),"action",action).unwrap().is_some());
    store.reader().execute("UPDATE object_instances SET identity_status='provisional'",[]).unwrap();
    assert!(cached(store.reader(),"action",action).unwrap().is_none());
    finish(&store);assert!(cached(store.reader(),"action",action).unwrap().is_some());
    assert_eq!(timeline(&store)[0]["context"]["identity_status"],"provisional");
}

#[test]
fn ordinary_new_event_bindings_do_not_invalidate_or_requeue_old_history() {
    let store=material();finish(&store);
    let action:i64=store.reader().query_row("SELECT min(action_row) FROM object_actions",[],|r|r.get(0)).unwrap();
    let before=cached(store.reader(),"action",action).unwrap().unwrap();
    store.reader().execute_batch("WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<1000)
        INSERT INTO source_bindings(instance_id,source,item_id,scope,native_record_id,valid_from)
        SELECT 1,'pcscope.eventlog.security','event-'||x,'machine:M','epoch-event-'||x,'2026-10-16T05:00:00Z' FROM n;").unwrap();
    assert_eq!(cached(store.reader(),"action",action).unwrap(),Some(before));
    assert_eq!(store.reader().query_row("SELECT count(*) FROM display_identity_work",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    assert_eq!(store.reader().query_row("SELECT count(*) FROM display_work WHERE kind='action'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
}

#[test]
fn newly_protected_names_do_not_reappear_from_historical_before_values() {
    let store=material();finish(&store);
    store.reader().execute("UPDATE object_facts SET value='\"******\"',value_status='masked' WHERE field_id='name'",[]).unwrap();
    finish(&store);
    for action in timeline(&store).as_array().unwrap() {
        assert!(action["context"]["subject_label"].is_null());
        assert!(action["context"]["presentation"]["name"].is_null());
    }
}

#[test]
fn scopes_use_exact_collection_addresses_and_history_recovery_does_not_cross_shards() {
    let store=material();
    crate::coverage::record_attempt(store.reader(),"pcscope.startup.tasks","machine:OTHER/schema:1/context:/shard:99",true,false,"2026-10-16T03:00:00Z").unwrap();
    let before=list(&store);assert_eq!(before[0]["coverage"].as_array().unwrap().len(),1);
    assert_eq!(before[0]["coverage"][0]["scope"],SCOPE);
    store.reader().execute_batch("DELETE FROM object_collection_scopes; UPDATE display_build SET evidence_cursor=0,evidence_bound=(SELECT max(seq) FROM evidence);").unwrap();
    finish(&store);assert_eq!(before,list(&store));
}

#[test]
fn presentation_rule_changes_keep_exact_scope_backfill_progress() {
    let store=material();finish(&store);
    let before:(i64,i64)=store.reader().query_row("SELECT evidence_cursor,evidence_bound FROM display_build",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
    store.reader().execute("UPDATE display_build SET revision='previous-display-rule'",[]).unwrap();
    ensure(store.reader()).unwrap();
    let after:(i64,i64)=store.reader().query_row("SELECT evidence_cursor,evidence_bound FROM display_build",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
    assert_eq!(after,before,"presentation changes do not change source scope evidence");
    finish(&store);
    assert_eq!(list(&store)[0]["coverage"][0]["scope"],SCOPE);
}

#[test]
fn rebuilding_timeline_uses_effective_time_order_without_sorting_the_full_history() {
    let store=material();
    let plan=store.reader().prepare("EXPLAIN QUERY PLAN SELECT action_row FROM object_actions a ORDER BY coalesce(a.interval_until,a.occurred_at,a.observed_at) DESC,a.action_row DESC LIMIT 201").unwrap()
        .query_map([],|r|r.get::<_,String>(3)).unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap().join(" ");
    assert!(plan.contains("object_actions_anchor"),"{plan}");assert!(!plan.contains("TEMP B-TREE"),"{plan}");
    let raw=timeline(&store);finish(&store);assert_eq!(raw,timeline(&store));
}

#[test]
fn progress_queries_do_not_visit_completed_queue_history() {
    use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};
    let store=material();
    store.reader().execute_batch("WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<50000)
        INSERT INTO projection_queue(evidence_id,item_id,projection,state,enqueued_at,updated_at)
        SELECT 'old-'||x,'one','object_model','done','2026-10-01','2026-10-01' FROM n;
        INSERT INTO projection_queue(evidence_id,item_id,projection,state,enqueued_at,updated_at)
        VALUES('pending-test','one','object_model','pending','2026-10-01','2026-10-01'),
              ('failed-test','one','object_model','failed','2026-10-01','2026-10-01');").unwrap();
    let steps=Arc::new(AtomicUsize::new(0));let counter=steps.clone();
    store.reader().progress_handler(1000,Some(move||counter.fetch_add(1000,Ordering::Relaxed)>20000));
    let result=store.query(&QueryRequest::ProjectionStatus).unwrap();
    for row in result.data.unwrap().as_array().unwrap(){if row["projection"]=="objects"{assert_eq!(row["pending"],1);assert_eq!(row["failed"],1);}}
    assert!(steps.load(Ordering::Relaxed)<=21000);
}

#[test]
fn appended_object_pages_can_omit_counts_without_changing_the_walk() {
    let store=material();let conn=store.reader();
    conn.execute_batch("INSERT INTO object_instances(type_id,category,machine_id,installation_id,scope,native_key,display_name,identity_status,lifecycle,first_observed_at,last_observed_at)
        VALUES('startup.scheduled_task','startup','M','I','machine:M','other','第二个任务','confirmed','active','2026-10-16','2026-10-16');").unwrap();
    let first=store.query(&QueryRequest::InstanceList(InstanceFilter{limit:1,..Default::default()})).unwrap();assert!(first.counts.is_some());
    let cursor=first.page.unwrap().next_cursor.unwrap();
    let with=store.query(&QueryRequest::InstanceList(InstanceFilter{limit:1,cursor:Some(cursor.clone()),..Default::default()})).unwrap();
    let without=store.query(&QueryRequest::InstanceList(InstanceFilter{limit:1,cursor:Some(cursor),include_counts:false,..Default::default()})).unwrap();
    assert_eq!(with.data,without.data);assert_eq!(with.page,without.page);assert!(without.counts.is_none());
}

#[test]
fn ordered_names_merge_dirty_new_and_missing_cache_rows_without_skips_or_duplicates() {
    use pcscope_contracts::query::InstanceSort;
    let store=material();let conn=store.reader();
    conn.execute_batch("WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<60)
        INSERT INTO object_instances(type_id,category,machine_id,installation_id,scope,native_key,display_name,identity_status,lifecycle,first_observed_at,last_observed_at)
        SELECT 'startup.scheduled_task','startup','M','I','machine:M','fast-'||x,CASE WHEN x%2=0 THEN '相同名称' ELSE '其他名称' END,'confirmed','active','2026-10-16','2026-10-16' FROM n;").unwrap();
    finish(&store);
    conn.execute_batch("UPDATE object_instances SET display_name='AAA changed' WHERE native_key='fast-10';
        DELETE FROM display_objects WHERE instance_id=(SELECT instance_id FROM object_instances WHERE native_key='fast-15');
        INSERT INTO object_instances(type_id,category,machine_id,installation_id,scope,native_key,display_name,identity_status,lifecycle,first_observed_at,last_observed_at)
        VALUES('startup.scheduled_task','startup','M','I','machine:M','fast-new','AAA new','confirmed','active','2026-10-16','2026-10-16');").unwrap();
    for sort in [InstanceSort::NameAsc,InstanceSort::NameDesc] {
        conn.execute("UPDATE display_build SET object_cursor=0",[]).unwrap();
        let expected=store.query(&QueryRequest::InstanceList(InstanceFilter{sort,limit:100,include_counts:false,..Default::default()})).unwrap().data.unwrap();
        conn.execute("UPDATE display_build SET object_cursor=object_bound",[]).unwrap();
        let mut visited=Vec::new();let mut cursor=None;
        for _ in 0..32 {
            let page=store.query(&QueryRequest::InstanceList(InstanceFilter{sort,limit:7,include_counts:false,cursor,..Default::default()})).unwrap();
            visited.extend(page.data.unwrap().as_array().unwrap().iter().cloned());
            cursor=page.page.unwrap().next_cursor;if cursor.is_none(){break;}
        }
        assert!(cursor.is_none(),"name paging did not reach its end");
        assert_eq!(Value::Array(visited),expected,"{sort:?}");
        let searched=store.query(&QueryRequest::InstanceList(InstanceFilter{sort,search:Some("AAA".into()),include_counts:false,..Default::default()})).unwrap();
        assert_eq!(searched.data.unwrap().as_array().unwrap().len(),2);
    }
}

#[test]
fn historical_rename_is_reconstructed_even_when_the_display_is_rebuilt_later() {
    let mut store=material();
    snapshot(&mut store,"renamed","2026-10-16T04:00:00Z","后来的名称",false,SCOPE);
    finish(&store);
    let actions=timeline(&store);let old=actions.as_array().unwrap().iter().find(|a|a["field_id"]=="enabled").unwrap();
    assert_eq!(old["context"]["subject_label"],"原来的名称");
    assert_eq!(list(&store)[0]["presentation"]["name"],"后来的名称");
    store.reader().execute_batch("UPDATE display_build SET revision='previous-rules';").unwrap();
    ensure(store.reader()).unwrap();finish(&store);
    let again=timeline(&store);assert_eq!(old,again.as_array().unwrap().iter().find(|a|a["field_id"]=="enabled").unwrap());
}

#[test]
fn warm_timeline_works_without_reopening_original_event_material() {
    let mut store=material();
    store.ingest(&pcscope_contracts::parse(&json!({"protocol_version":1,"request_id":"events","type":"event_batch",
      "source":"pcscope.eventlog.security","epoch":"fixture","batch_seq":1,"checkpoint":"1",
      "events":[{"id":"event-1","kind":"identity.logon_failed","object_id":"","occurred_at":"2026-10-16T03:00:00Z",
      "data":{"provider":"Microsoft-Windows-Security-Auditing","event_id":4625,"fields":{"TargetUserName":"历史账户"}}}]}).to_string()).unwrap()).unwrap();
    store.project_pending("M","I",100).unwrap();let expected=timeline(&store);finish(&store);
    assert!(expected.as_array().unwrap().iter().any(|a|a["context"]["subject_label"]=="历史账户"));
    // A cold read would prepare action_context's records join and fail. Warm queries use only
    // stored views; deleting material here is confined to this disposable in-memory fixture.
    store.reader().execute_batch("DROP TABLE records;").unwrap();
    assert_eq!(expected,timeline(&store));
}

#[test]
fn prepared_generation_prevents_rebuilt_old_rows_from_entering_an_existing_page_walk() {
    let store=material();
    store.reader().execute("DELETE FROM object_actions",[]).unwrap();
    for hour in [1,2,3] {
        store.reader().execute("INSERT INTO object_actions(instance_id,action_id,result,precision,observed_at,rule_version) VALUES(1,'fixture.action','observed','observation_only',?,'test')",[format!("2026-10-16T0{hour}:00:00Z")]).unwrap();
    }
    finish(&store);
    let middle:i64=store.reader().query_row("SELECT action_row FROM object_actions ORDER BY observed_at LIMIT 1 OFFSET 1",[],|r|r.get(0)).unwrap();
    store.reader().execute("INSERT INTO display_work(kind,id) VALUES('action',?)",[middle]).unwrap();
    let first=store.query(&QueryRequest::ActionTimeline(ActionFilter{limit:1,..Default::default()})).unwrap();
    assert_eq!(first.data.as_ref().unwrap()[0]["observed_at"],"2026-10-16T03:00:00Z");
    let cursor=first.page.unwrap().next_cursor;assert!(cursor.is_some());finish(&store);
    let next=store.query(&QueryRequest::ActionTimeline(ActionFilter{limit:1,cursor,..Default::default()})).unwrap();
    assert_eq!(next.data.as_ref().unwrap()[0]["observed_at"],"2026-10-16T01:00:00Z");
    assert_eq!(timeline(&store).as_array().unwrap().len(),3,"fresh queries see the rebuilt record");
}

#[test]
fn distinct_counts_handle_shared_evidence_changes_deletes_and_rollback() {
    let store=material();finish(&store);
    let exact=|store:&Store| {
        let fast=store.query(&QueryRequest::InstanceList(InstanceFilter::default())).unwrap().counts.unwrap();
        let slow=store.query(&QueryRequest::InstanceList(InstanceFilter{search:Some(String::new()),..Default::default()})).unwrap().counts.unwrap();
        assert_eq!(fast,slow);fast
    };
    exact(&store);
    let conn=store.reader();
    conn.execute("INSERT INTO evidence_refs VALUES('observation','1','snapshot','same-address',NULL)",[]).unwrap();
    let count=exact(&store).evidence;
    conn.execute("INSERT INTO evidence_refs VALUES('observation','1','event','same-address',NULL)",[]).unwrap();
    assert_eq!(exact(&store).evidence,count,"one address cited twice is still one evidence item");
    {let tx=conn.unchecked_transaction().unwrap();tx.execute("DELETE FROM evidence_refs WHERE evidence_id='same-address'",[]).unwrap();}
    assert_eq!(exact(&store).evidence,count);
    conn.execute("DELETE FROM evidence_refs WHERE evidence_id='same-address' AND evidence_kind='event'",[]).unwrap();assert_eq!(exact(&store).evidence,count);
    conn.execute("UPDATE source_bindings SET valid_to='2026-10-16'",[]).unwrap();exact(&store);
    conn.execute("DELETE FROM object_instances",[]).unwrap();assert_eq!(exact(&store).instances,0);
}

#[test]
fn restart_preserves_build_cursors_and_invalid_revision_never_serves_old_views() {
    let task_path=std::env::temp_dir().join(format!("pcscope-display-{}-{}.db",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let cursor;
    {
        let mut store=Store::open(&task_path).unwrap();snapshot(&mut store,"one","2026-10-16T01:00:00Z","名称",true,SCOPE);
        ensure(store.reader()).unwrap(); // unchanged version does not reset maintenance state.
        store.prepare_display(1,Duration::from_millis(1)).unwrap();
        cursor=status(store.reader()).unwrap();
    }
    let store=Store::open(&task_path).unwrap();assert_eq!(cursor,status(store.reader()).unwrap());finish(&store);
    store.reader().execute("UPDATE display_objects SET revision='old'",[]).unwrap();assert!(cached(store.reader(),"object",1).unwrap().is_none());
    drop(store);std::fs::remove_file(task_path).unwrap();
}
