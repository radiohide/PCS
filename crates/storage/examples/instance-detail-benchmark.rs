//! Read-only detail latency check; accepts explicit instance IDs without copying history.
use anyhow::{ensure,Result};
use pcscope_contracts::query::QueryRequest;
use pcscope_storage::Store;
use serde_json::json;
use std::time::{Duration,Instant};
fn main()->Result<()> {
    let args:Vec<_>=std::env::args().collect();ensure!(args.len()>=3,"instance-detail-benchmark database instance-id [instance-id ...]");
    let mut results=Vec::new();
    for id in &args[2..] {
        let mut times=Vec::new();let mut size=0;
        for _ in 0..30 {
            let store=Store::open_reader(&args[1],Duration::from_secs(6))?;
            let start=Instant::now();let response=store.query(&QueryRequest::InstanceDetail{instance:id.clone()})?;
            times.push(start.elapsed().as_secs_f64()*1000.);size=serde_json::to_vec(&response)?.len();
        }
        let cold=times[0];times.sort_by(f64::total_cmp);
        results.push(json!({"instance":id,"first_ms":cold,"p50_ms":times[15],"p95_ms":times[28],"max_ms":times[29],"response_bytes":size}));
    }
    println!("{}",serde_json::to_string_pretty(&results)?);Ok(())
}
