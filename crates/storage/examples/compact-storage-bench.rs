//! Same synthetic workload can be built against the previous release for a fair comparison.
//! Creates only a new database at the supplied path; never reads production history.
use anyhow::{ensure,Result};
use pcscope_storage::Store;
use pcscope_contracts::query::{QueryRequest,ActionFilter};
use serde_json::{json,Value};
use std::{path::PathBuf,time::{Instant,Duration}};
fn stats(mut values:Vec<f64>)->Value{values.sort_by(f64::total_cmp);json!({"p50_ms":values[values.len()/2],"p95_ms":values[(values.len()*95/100).min(values.len()-1)],"total_ms":values.iter().sum::<f64>()})}
fn main()->Result<()> {
    let path=PathBuf::from(std::env::args().nth(1).expect("new fixture database path"));
    ensure!(!path.exists(),"refusing existing database");
    let rounds:usize=std::env::args().nth(2).unwrap_or("60".into()).parse()?;ensure!((10..=1000).contains(&rounds),"10..1000 rounds");
    let count=100;let mut store=Store::open(&path)?;let mut ingest=Vec::new();let mut projection=Vec::new();
    let mut first_size=0;
    for n in 0..rounds {
        let time=chrono::DateTime::parse_from_rfc3339("2026-10-23T00:00:00Z")?+chrono::Duration::minutes(n as i64*10);
        let state=if n==rounds/2{"Stopped"}else{"Running"};
        let envelope=pcscope_contracts::parse(&json!({"protocol_version":1,"request_id":n.to_string(),"type":"snapshot","source":"pcscope.startup.services","scope":"machine:fixture","run_id":n.to_string(),"observed_at":time.to_rfc3339(),"complete":true,
            "items":(0..count).map(|i|json!({"id":format!("service-{i}"),"data":{"name":format!("service-{i}"),"display_name":format!("Fixture service {i}"),"state":state,"description":"unchanged description ".repeat(70)}})).collect::<Vec<_>>()}).to_string())?;
        let start=Instant::now();store.ingest(&envelope)?;ingest.push(start.elapsed().as_secs_f64()*1000.0);
        let start=Instant::now();while store.project_pending("fixture","fixture",256)?.examined>0{}projection.push(start.elapsed().as_secs_f64()*1000.0);
        if n==0{first_size=store.reader().query_row("SELECT page_count*page_size FROM pragma_page_count,pragma_page_size",[],|r|r.get::<_,i64>(0))?;}
    }
    for _ in 0..100 {if store.prepare_display(256,Duration::from_millis(100))?["pending"]==false{break}}
    let compare=QueryRequest::StateCompare{from:"2026-10-23T00:00:00Z".into(),until:"2026-10-23T23:59:00Z".into(),type_id:Some("startup.service".into()),cursor:None,limit:50,watermark:None};
    let mut timings=serde_json::Map::new();let mut samples=serde_json::Map::new();
    for (name,request) in [("timeline",QueryRequest::ActionTimeline(ActionFilter{limit:200,..Default::default()})),("comparison",compare),("health",QueryRequest::ProjectionStatus)]{
        let mut values=Vec::new();for _ in 0..20{let now=Instant::now();let answer=store.query(&request)?;values.push(now.elapsed().as_secs_f64()*1000.0);samples.insert(name.into(),serde_json::to_value(answer)?);}
        timings.insert(name.into(),stats(values));
    }
    let schema:i64=store.reader().query_row("PRAGMA user_version",[],|r|r.get(0))?;
    let table=if schema>=12{"all_evidence"}else{"evidence"};
    let evidence:i64=store.reader().query_row(&format!("SELECT count(*) FROM {table}"),[],|r|r.get(0))?;
    let facts:i64=store.reader().query_row("SELECT count(*) FROM object_facts",[],|r|r.get(0))?;
    let actions:i64=store.reader().query_row("SELECT count(*) FROM object_actions",[],|r|r.get(0))?;
    let usage=store.retention()?;
    store.reader().execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
    ensure!(store.reader().query_row("PRAGMA integrity_check",[],|r|r.get::<_,String>(0))?=="ok","integrity");
    ensure!(store.reader().prepare("PRAGMA foreign_key_check")?.query([])?.next()?.is_none(),"foreign keys");
    let size=std::fs::metadata(&path)?.len();
    let output=json!({"version":env!("CARGO_PKG_VERSION"),"schema":schema,"rounds":rounds,"items_per_round":count,"fixture":"unchanged services except one stopped round then reversion","production_access":false,"first_round_bytes":first_size,"database_bytes":size,"growth_per_round":(size as i64-first_size)/(rounds as i64-1),"evidence":evidence,"facts":facts,"actions":actions,"ingest":stats(ingest),"projection":stats(projection),"queries":timings,"usage":usage,"samples":samples});
    std::fs::write(path.with_extension("json"),serde_json::to_vec_pretty(&output)?)?;
    println!("{}",output);Ok(())
}
