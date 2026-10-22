//! Read a minimized fixture; all mutation is confined to a new in-memory database.
use anyhow::{ensure,Result};
use pcscope_storage::Store;
use serde_json::{json,Value};
use std::time::{Duration,Instant};
fn main()->Result<()> {
    let path=std::env::args().nth(1).ok_or_else(||anyhow::anyhow!("retained-history-replay fixture.json"))?;
    let items:Vec<Value>=serde_json::from_str(&std::fs::read_to_string(path)?)?;
    let mut store=Store::open(":memory:")?;let installation=store.projection_installation("fixture")?;
    store.ingest(&pcscope_contracts::parse(&json!({"protocol_version":1,"request_id":"fixture","type":"snapshot","source":"pcscope.system.update-history","scope":"machine:fixture","run_id":"fixture","observed_at":"2026-10-22T00:00:00Z","complete":true,"items":items}).to_string())?)?;
    while store.project_pending("fixture",&installation,128)?.examined>0 {}
    let count=|sql:&str|store.reader().query_row(sql,[],|r|r.get::<_,i64>(0));
    let before=count("SELECT count(*) FROM object_actions WHERE action_id='software.update_history'")?;
    ensure!(before==items.len() as i64,"missing history actions");
    store.reader().execute("UPDATE object_actions SET instance_id=NULL,unassigned_reason='legacy independent history',target_type='software.system_update' WHERE action_id='software.update_history'",[])?;
    let start=Instant::now();let mut times=Vec::new();
    loop {let batch=Instant::now();let r=store.repair_retained_history(128,Duration::from_millis(40))?;times.push(batch.elapsed().as_secs_f64()*1000.);if r["pending"]==false {break}}
    let elapsed=start.elapsed().as_secs_f64()*1000.;
    let linked=count("SELECT count(*) FROM object_actions WHERE action_id='software.update_history' AND instance_id IS NOT NULL")?;
    let subjects=count("SELECT count(*) FROM object_instances WHERE type_id='software.update_history_subject'")?;
    store.reader().execute("UPDATE projection_queue SET state='pending'",[])?;
    while store.project_pending("fixture",&installation,128)?.examined>0 {}
    ensure!(count("SELECT count(*) FROM object_actions WHERE action_id='software.update_history'")?==before,"replay duplicated history");
    ensure!(count("SELECT count(*) FROM object_instances WHERE lifecycle='active'")?==0,"history asserted current installation");
    times.sort_by(f64::total_cmp);
    println!("{}",json!({"actions":before,"linked":linked,"historical_subjects":subjects,"repair_ms":elapsed,"batch_p50_ms":times[times.len()/2],"batch_p95_ms":times[times.len()*95/100],"batch_max_ms":times.last(),"idempotent":true,"live_objects_created":0}));Ok(())
}
