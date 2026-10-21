//! Offline retained-event replay. All writes are confined to a new in-memory database.
use anyhow::{ensure,Result};
use pcscope_storage::Store;
use serde_json::{json,Value};
use std::time::{Duration,Instant};
fn main()->Result<()> {
    let args:Vec<_>=std::env::args().collect();ensure!(args.len()==4,"event-association-replay fixture.json boot-start observed-until");
    let events:Vec<Value>=serde_json::from_str(&std::fs::read_to_string(&args[1])?)?;
    let mut store=Store::open(":memory:")?;let installation=store.projection_installation("replay-machine")?;
    let mut sequences=std::collections::BTreeMap::<String,u64>::new();
    for (index,item) in events.iter().enumerate() {
        let seq=sequences.entry(item["source"].as_str().unwrap().to_owned()).or_default();*seq+=1;
        let envelope=json!({"protocol_version":1,"request_id":index.to_string(),"type":"event_batch","source":item["source"],"epoch":"replay","batch_seq":*seq,"checkpoint":index.to_string(),"events":[item["event"]]});
        store.ingest(&pcscope_contracts::parse(&envelope.to_string())?)?;
        if index%128==127{while store.project_pending("replay-machine",&installation,128)?.examined>0{}}
    }
    while store.project_pending("replay-machine",&installation,128)?.examined>0{}
    let c=store.reader();let count:i64=c.query_row("SELECT count(*) FROM object_actions",[],|r|r.get(0))?;
    ensure!(count==events.len() as i64,"source events did not all produce actions");
    c.execute("INSERT INTO installation_boots(machine_id,installation_id,boot_id,start_at,observed_until,evidence_id) VALUES(?,?,'11111111-1111-1111-1111-111111111111',?,?,'fixture-boot')",rusqlite::params!["replay-machine",installation,pcscope_contracts::semantics::canonical_time(&args[2]).map_err(anyhow::Error::msg)?,pcscope_contracts::semantics::canonical_time(&args[3]).map_err(anyhow::Error::msg)?])?;
    let started=Instant::now();let mut times=Vec::new();let report=loop {
        let start=Instant::now();let result=store.repair_event_context(128,Duration::from_millis(40))?;times.push(start.elapsed().as_secs_f64()*1000.);
        if result["pending"]==false{break result}
    };
    let elapsed=started.elapsed().as_secs_f64()*1000.;
    ensure!(c.query_row("SELECT count(*) FROM object_actions",[],|r|r.get::<_,i64>(0))?==count,"history repair changed action count");
    c.execute("UPDATE projection_queue SET state='pending'",[])?;
    while store.project_pending("replay-machine",&installation,128)?.examined>0{}
    ensure!(c.query_row("SELECT count(*) FROM object_actions",[],|r|r.get::<_,i64>(0))?==count,"replay duplicated actions");
    let mut q=c.prepare("SELECT action_id,count(*) AS total,count(instance_id) AS linked FROM object_actions GROUP BY action_id")?;
    let counts=q.query_map([],|r|Ok(json!({"action":r.get::<_,String>(0)?,"total":r.get::<_,i64>(1)?,"linked":r.get::<_,i64>(2)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;
    times.sort_by(f64::total_cmp);
    println!("{}",serde_json::to_string_pretty(&json!({"events":count,"repair_ms":elapsed,"repair":report,"counts":counts,"batch_p50_ms":times[times.len()/2],"batch_p95_ms":times[times.len()*95/100],"batch_max_ms":times.last(),"replay_idempotent":true}))?);Ok(())
}
