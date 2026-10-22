//! Read-only acceptance against an explicitly supplied database. Never invokes collectors.
use anyhow::Result;
use pcscope_contracts::query::QueryRequest;
use pcscope_storage::Store;
use serde_json::{json,Value};
use std::time::{Duration,Instant};
fn main()->Result<()> {
    let args:Vec<String>=std::env::args().collect();anyhow::ensure!(args.len()==4,"state-compare <database> <from UTC> <until UTC>");
    let store=Store::open_reader(&args[1],Duration::from_secs(6))?;
    let started=Instant::now();
    let request:QueryRequest=serde_json::from_value(json!({"query":"state_compare","from":args[2],"until":args[3]}))?;
    let directory=store.query(&request)?;let directory_ms=started.elapsed().as_millis();
    let data=directory.data.as_ref().unwrap();let mut pages=Vec::new();
    let timed=["startup.service","hardware.device","network.adapter","runtime.module_instance","software.installation_registration","account.user","startup.scheduled_task"];
    let mut types:Vec<&str>=timed.to_vec();
    for entry in data["branches"].as_array().unwrap(){let typ=entry["type_id"].as_str().unwrap();if !types.contains(&typ){types.push(typ)}}
    for typ in types {
        let store=Store::open_reader(&args[1],Duration::from_secs(6))?;
        let request:QueryRequest=serde_json::from_value(json!({"query":"state_compare","from":args[2],"until":args[3],"watermark":data["watermark"],"type_id":typ,"limit":if timed.contains(&typ){50}else{1}}))?;
        let started=Instant::now();let result=store.query(&request);
        match result {Ok(result)=>pages.push(json!({"type_id":typ,"ms":started.elapsed().as_millis(),"response":result})),Err(error)=>pages.push(json!({"type_id":typ,"ms":started.elapsed().as_millis(),"error":format!("{error:#}")}))}
    }
    let failed=pages.iter().any(|p|!p["error"].is_null());
    println!("{}",json!({"directory_ms":directory_ms,"directory":directory,"pages":Value::Array(pages)}));
    anyhow::ensure!(!failed,"some comparison pages failed");Ok(())
}
