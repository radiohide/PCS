use crate::{Store,compact,evidence,storage_metrics};
use serde_json::json;

fn snapshot(run:usize,source:&str,scope:&str,n:usize,value:&str)->pcscope_contracts::Envelope {
    pcscope_contracts::parse(&json!({"protocol_version":1,"request_id":run.to_string(),"type":"snapshot","source":source,"scope":scope,"run_id":run.to_string(),"observed_at":format!("2026-10-26T{:02}:{:02}:00Z",run/60,run%60),"complete":true,
        "items":(0..n).map(|i|json!({"id":format!("service-{i}"),"data":{"name":format!("service-{i}"),"display_name":"服务","state":value,"description":"甲".repeat(600)}})).collect::<Vec<_>>()}).to_string()).unwrap()
}
fn count(s:&Store,table:&str)->i64{s.reader().query_row(&format!("SELECT count(*) FROM {table}"),[],|r|r.get(0)).unwrap()}
fn drain(s:&Store){while s.project_pending("M","I",50).unwrap().examined>0 {}}
#[test] fn source_filter_does_not_expand_unrelated_snapshot_history(){
    use pcscope_contracts::query::{QueryRequest,ActionFilter};
    let mut s=Store::open(":memory:").unwrap();let source="pcscope.startup.services";
    s.ingest(&snapshot(0,source,"machine:M",1,"Running")).unwrap();drain(&s);
    s.ingest(&snapshot(1,source,"machine:M",1,"Stopped")).unwrap();drain(&s);
    let items:Vec<_>=(0..100).map(|i|pcscope_contracts::Item{id:i.to_string(),data:json!({})}).collect();
    s.reader().execute_batch("BEGIN").unwrap();
    for n in 0..1000 {compact::store(s.reader(),"unrelated","scope",&n.to_string(),"now","now",&items,"{}").unwrap();}
    s.reader().execute_batch("COMMIT").unwrap();
    use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};
    let steps=Arc::new(AtomicUsize::new(0));let used=steps.clone();
    s.reader().progress_handler(100,Some(move||used.fetch_add(100,Ordering::Relaxed)>10000));
    let result=s.query(&QueryRequest::ActionTimeline(ActionFilter{source:Some(source.into()),limit:20,..Default::default()})).unwrap();
    assert_eq!(result.data.unwrap().as_array().unwrap().len(),1);
    assert!(steps.load(Ordering::Relaxed)<10000);
}
fn counters(s:&Store){
    let c=s.reader();
    let expected:Vec<(String,String,i64)>=c.prepare("SELECT e.source,q.state,count(*) FROM all_projection_queue q JOIN all_evidence e ON e.evidence_id=q.evidence_id AND e.item_id=q.item_id GROUP BY e.source,q.state ORDER BY e.source,q.state").unwrap().query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap().collect::<rusqlite::Result<_>>().unwrap();
    let actual:Vec<(String,String,i64)>=c.prepare("SELECT source,state,sum(item_count) FROM (SELECT * FROM compact_counts UNION ALL SELECT * FROM coverage_projection_counts) GROUP BY source,state HAVING sum(item_count)>0 ORDER BY source,state").unwrap().query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap().collect::<rusqlite::Result<_>>().unwrap();
    assert_eq!(actual,expected);
}
#[test] fn shared_members_preserve_every_check_change_reversion_scope_and_replay(){
    let mut s=Store::open(":memory:").unwrap();let source="pcscope.startup.services";
    for run in 0..12 {s.ingest(&snapshot(run,source,"machine:M",20,if run==6{"Stopped"}else{"Running"})).unwrap();drain(&s);}
    assert_eq!(count(&s,"observation_sets"),1);assert_eq!(count(&s,"observation_members"),20);
    assert_eq!(count(&s,"observation_runs"),12);assert_eq!(count(&s,"projection_queue"),0);
    assert_eq!(count(&s,"snapshot_contents"),2);
    let address=format!("{source}|machine:M|11");
    let input=evidence::input_at(s.reader(),&address,"service-0").unwrap().unwrap();
    assert_eq!(input.observed_at,"2026-10-26T00:11:00Z");assert_eq!(input.payload["state"],"Running");
    let before=compact::watermark(s.reader()).unwrap();
    assert_eq!(s.ingest(&snapshot(11,source,"machine:M",20,"Running")).unwrap(),0);
    assert_eq!(compact::watermark(s.reader()).unwrap(),before);
    s.ingest(&snapshot(12,source,"machine:N",20,"Running")).unwrap();drain(&s);
    assert_eq!(count(&s,"observation_sets"),1);assert_eq!(count(&s,"object_instances"),40);
    s.ingest(&snapshot(13,source,"machine:M",0,"Running")).unwrap();drain(&s);
    s.ingest(&snapshot(14,source,"machine:M",20,"Running")).unwrap();drain(&s);
    counters(&s);
}
#[test] fn partial_progress_errors_requeue_and_rollback_preserve_counters(){
    let mut s=Store::open(":memory:").unwrap();s.ingest(&snapshot(0,"pcscope.unknown.fixture","machine:M",5,"Running")).unwrap();
    counters(&s);
    s.project_pending("M","I",2).unwrap();counters(&s);
    assert_eq!(count(&s,"observation_errors"),2);
    s.reader().execute_batch("BEGIN").unwrap();
    let next=compact::pending(s.reader(),evidence::OBJECT_PROJECTION,1).unwrap()[0].0;
    compact::finish(s.reader(),next,"done",None).unwrap();counters(&s);
    s.reader().execute_batch("ROLLBACK").unwrap();counters(&s);
    drain(&s);assert_eq!(count(&s,"observation_errors"),5);counters(&s);
    s.reader().execute("UPDATE all_projection_queue SET state='pending' WHERE state='failed'",[]).unwrap();
    assert_eq!(count(&s,"observation_errors"),0);counters(&s);
    drain(&s);assert_eq!(count(&s,"observation_errors"),5);counters(&s);
}
#[test] fn content_accounting_is_exact_transactional_and_shared_once(){
    let mut s=Store::open(":memory:").unwrap();let source="pcscope.startup.services";
    for n in 0..3{s.ingest(&snapshot(n,source,"machine:M",3,"Running")).unwrap();}
    let usage=storage_metrics::report(s.reader()).unwrap();
    let expected:i64=s.reader().query_row("SELECT (SELECT coalesce(sum(length(CAST(payload AS BLOB))),0) FROM snapshots)+(SELECT coalesce(sum(length(CAST(items AS BLOB))),0) FROM snapshot_contents)+(SELECT coalesce(sum(length(CAST(data AS BLOB))),0) FROM records)",[],|r|r.get(0)).unwrap();
    assert_eq!(usage["sources"][0]["retained_bytes"],expected);
    s.ingest(&snapshot(2,source,"machine:M",3,"Running")).unwrap();assert_eq!(storage_metrics::report(s.reader()).unwrap(),usage);
    s.reader().execute_batch("BEGIN").unwrap();
    s.reader().execute("DELETE FROM snapshots WHERE run_id='0'",[]).unwrap();assert_eq!(count(&s,"snapshot_contents"),1);
    s.reader().execute_batch("ROLLBACK").unwrap();assert_eq!(storage_metrics::report(s.reader()).unwrap(),usage);
    let unit:i64=s.reader().query_row("SELECT unit_id FROM snapshots WHERE run_id='0'",[],|r|r.get(0)).unwrap();
    evidence::mark_pruned_for_unit(s.reader(),unit,"2026-10-27").unwrap();
    assert_eq!(s.reader().query_row("SELECT pruned_at FROM observation_runs WHERE run_id='0'",[],|r|r.get::<_,String>(0)).unwrap(),"2026-10-27");
    s.reader().execute_batch("DELETE FROM snapshots; DELETE FROM records;").unwrap();
    assert_eq!(count(&s,"snapshot_contents"),0);assert_eq!(storage_metrics::report(s.reader()).unwrap()["sources"][0]["retained_bytes"],0);
    assert_eq!(storage_metrics::report(s.reader()).unwrap()["sources"][0]["written_bytes"],expected);
}
#[test] fn backlog_and_health_reads_are_bounded_independent_of_completed_members(){
    let s=Store::open(":memory:").unwrap();let c=s.reader();
    let items=vec![pcscope_contracts::Item{id:"one".into(),data:json!({})}];
    c.execute_batch("BEGIN").unwrap();
    for n in 0..1500 {compact::store(c,"source","scope",&n.to_string(),"now","now",&items,"{}").unwrap();compact::finish(c,-compact::watermark(c).unwrap(),"done",None).unwrap();}
    c.execute_batch("COMMIT").unwrap();
    compact::store(c,"source","scope","pending","now","now",&items,"{}").unwrap();
    use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};
    let steps=Arc::new(AtomicUsize::new(0));let used=steps.clone();
    c.progress_handler(100,Some(move||used.fetch_add(100,Ordering::Relaxed)>5000));
    assert_eq!(compact::pending(c,evidence::OBJECT_PROJECTION,10).unwrap().len(),1);
    assert_eq!(evidence::queue_summary(c,evidence::OBJECT_PROJECTION).unwrap()["by_state"]["done"],1500);
    storage_metrics::report(c).unwrap();assert!(steps.load(Ordering::Relaxed)<5000);
}
#[test] fn reopen_resumes_mid_batch_without_repeating_committed_items(){
    let dir=std::env::temp_dir().join(format!("pcscope-compact-{}",std::process::id()));std::fs::create_dir_all(&dir).unwrap();
    let path=dir.join(format!("{}.db",std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    {let mut s=Store::open(&path).unwrap();s.ingest(&snapshot(0,"pcscope.startup.services","machine:M",5,"Running")).unwrap();s.project_pending("M","I",2).unwrap();}
    {let s=Store::open(&path).unwrap();assert_eq!(compact::pending(s.reader(),evidence::OBJECT_PROJECTION,10).unwrap().len(),3);drain(&s);assert_eq!(count(&s,"object_instances"),5);counters(&s);}
    std::fs::remove_file(path).unwrap();
}
