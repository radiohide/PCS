//! Read a small, explicitly exported fixture; all projection/repair writes stay in memory.
use anyhow::Result;
use pcscope_storage::Store;
use serde_json::{json,Value};
use std::time::{Duration,Instant};
fn main()->Result<()> {
    let path=std::env::args().nth(1).ok_or_else(||anyhow::anyhow!("provide the exported process fixture"))?;
    let inputs:Vec<Value>=serde_json::from_slice(&std::fs::read(path)?)?;
    let mut store=Store::open(":memory:")?;store.projection_installation("replay")?;
    let installation:String=store.reader().query_row("SELECT value FROM settings WHERE key='projection_installation'",[],|r|r.get(0))?;
    for (index,item) in inputs.iter().enumerate() {
        store.ingest(&pcscope_contracts::parse(&json!({"protocol_version":1,"request_id":index.to_string(),"type":"event_batch","source":item["source"],"epoch":"isolated-replay","batch_seq":index+1,"checkpoint":index.to_string(),"events":[{"id":index.to_string(),"object_id":index.to_string(),"kind":item["item_id"],"occurred_at":item["source_time"],"data":item["payload"]}]}).to_string())?)?;
        while store.project_pending("replay",&installation,100)?.examined>0{}
    }
    let c=store.reader();let scalar=|sql:&str|c.query_row(sql,[],|r|r.get::<_,i64>(0));
    let rows=scalar("SELECT count(*) FROM object_actions")?;
    anyhow::ensure!(rows==inputs.len() as i64,"fixture changed action count");
    c.execute("UPDATE object_actions SET instance_id=NULL,unassigned_reason='legacy missing creation time'",[])?;
    let start=Instant::now();let mut times=Vec::new();
    loop {let at=Instant::now();let r=store.repair_process_history(128,Duration::from_millis(40))?;times.push(at.elapsed().as_secs_f64()*1000.);if r["pending"]==false{break}}
    let elapsed=start.elapsed().as_secs_f64()*1000.;
    let linked=scalar("SELECT count(*) FROM object_actions WHERE instance_id IS NOT NULL")?;
    anyhow::ensure!(linked==rows,"not all supported process fixtures linked");
    anyhow::ensure!(scalar("SELECT count(*) FROM object_facts WHERE field_id='created_at'")?==0,"fabricated native creation time");
    anyhow::ensure!(scalar("SELECT count(*) FROM object_instances WHERE lifecycle='active'")?==0,"history created current process");
    anyhow::ensure!(store.repair_process_history(128,Duration::from_millis(40))?["pending"]==false,"repair not complete");
    times.sort_by(f64::total_cmp);
    println!("{}",serde_json::to_string_pretty(&json!({"actions":rows,"linked":linked,"historical_processes":scalar("SELECT count(*) FROM object_instances")?,"repair_ms":elapsed,"batch_p50_ms":times[times.len()/2],"batch_max_ms":times.last(),"idempotent":true,"native_creation_times_fabricated":0}))?);
    Ok(())
}
