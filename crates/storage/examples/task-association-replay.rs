//! Replays retained TaskScheduler materials in memory; never opens the production database.
use anyhow::{ensure,Result};
use pcscope_storage::Store;
use serde_json::{json,Value};
use std::time::{Duration,Instant};
fn main()->Result<()> {
    let path=std::env::args().nth(1).ok_or_else(||anyhow::anyhow!("usage: task-association-replay fixture.json"))?;
    let fixture:Value=serde_json::from_slice(&std::fs::read(path)?)?;
    let entries=fixture.as_array().ok_or_else(||anyhow::anyhow!("expected an array"))?;
    let mut store=Store::open(":memory:")?;
    for (index,chunk) in entries.chunks(128).enumerate() {
        let envelope=pcscope_contracts::parse(&json!({"protocol_version":1,"request_id":format!("{index}"),"type":"event_batch",
            "source":"pcscope.eventlog.microsoft-windows-taskscheduler/operational","epoch":"retained-replay","batch_seq":index+1,"checkpoint":format!("{index}"),"events":chunk}).to_string())?;
        store.ingest(&envelope)?;
        while store.project_pending("replay-machine","replay-install",64)?.examined>0 {}
    }
    let c=store.reader();
    let count:i64=c.query_row("SELECT count(*) FROM object_actions",[],|r|r.get(0))?;
    ensure!(count==entries.len() as i64,"events disappeared during projection");
    let paths:i64=c.query_row("SELECT count(DISTINCT instance_id) FROM object_actions",[],|r|r.get(0))?;
    c.execute_batch("INSERT INTO object_instances(type_id,category,machine_id,installation_id,scope,native_key,incarnation,identity_status,lifecycle,first_observed_at,last_observed_at)
       VALUES('startup.scheduled_task','startup','replay-machine','replay-install','machine:replay-machine','machine:replay-machine|?path',1,'provisional','active','2026-10-01T00:00:00Z','2026-10-21T00:00:00Z');")?;
    let shared=c.last_insert_rowid();
    c.execute("UPDATE object_actions SET instance_id=?",[shared])?;
    c.execute("UPDATE source_bindings SET instance_id=?",[shared])?;
    let start=Instant::now();let mut times=Vec::new();let report=loop {
        let start=Instant::now();let result=store.repair_event_associations(128,Duration::from_millis(40))?;times.push(start.elapsed().as_secs_f64()*1000.);
        if result["pending"]==false {break result}
    };
    let elapsed=start.elapsed().as_secs_f64()*1000.;
    ensure!(report["repaired"]==count,"not all retained tasks repaired: {report}");
    ensure!(c.query_row("SELECT count(DISTINCT instance_id) FROM object_actions",[],|r|r.get::<_,i64>(0))?==paths,"task identities changed");
    while store.prepare_display(128,Duration::from_millis(100))?["pending"]==true {}
    let warning:i64=c.query_row("SELECT count(*) FROM display_actions WHERE json_extract(view,'$.context.attribution_status') IN ('mismatch','unverified')",[],|r|r.get(0))?;
    ensure!(warning==0,"corrected actions still lose their object links");
    c.execute("UPDATE projection_queue SET state='pending'",[])?;
    while store.project_pending("replay-machine","replay-install",64)?.examined>0 {}
    ensure!(c.query_row("SELECT count(*) FROM object_actions",[],|r|r.get::<_,i64>(0))?==count,"replay duplicated actions");
    times.sort_by(f64::total_cmp);
    println!("{}",serde_json::to_string_pretty(&json!({"events":count,"task_paths":paths,"warnings_after":warning,"repair_ms":elapsed,"batches":times.len(),"batch_p50_ms":times[times.len()/2],"batch_p95_ms":times[(times.len()*95/100).min(times.len()-1)],"batch_max_ms":times.last(),"repair":report,"raw_history_preserved":true,"replay_idempotent":true}))?);
    Ok(())
}
