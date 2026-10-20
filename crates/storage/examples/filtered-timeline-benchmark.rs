//! Bounded, read-only checks of sparse and dense filters against the actual history.
use anyhow::{ensure,Result};
use pcscope_contracts::query::{ActionFilter,QueryRequest};
use pcscope_storage::Store;
use serde_json::json;
use std::time::{Duration,Instant};
fn main()->Result<()> {
    let args:Vec<_>=std::env::args().collect();
    ensure!(args.len()==3,"filtered-timeline-benchmark <read-only-db> <metrics-output>");
    let cases=[
        ("hardware",Some("hardware"),None,None,false),
        ("hardware_all_updates",Some("hardware"),None,None,true),
        ("hardware_device",None,Some("hardware.device"),None,false),
        ("hardware_empty",Some("hardware"),Some("runtime.process"),None,false),
        ("software",Some("software"),None,None,false),
        ("security",Some("security"),None,None,false),
        ("startup",Some("startup"),None,None,false),
        ("network_dense",Some("network"),None,None,false),
        ("runtime_dense",Some("runtime"),None,None,false),
        ("unfiltered",None,None,None,false),
        ("failed_logons",None,None,Some("identity.logon_failed"),false),
        ("failed_logons_account",Some("account"),None,Some("identity.logon_failed"),false),
        ("failed_logons_user",None,Some("account.user"),Some("identity.logon_failed"),false),
    ];
    let mut metrics=Vec::new();
    for (name,category,typ,action,routine) in cases {
        for round in 0..2 {
            let store=Store::open_reader(&args[1],Duration::from_secs(6))?;
            let filter=ActionFilter{category:category.map(str::to_owned),type_id:typ.map(str::to_owned),action_id:action.map(str::to_owned),include_routine:routine,cards_only:true,limit:400,..Default::default()};
            let start=Instant::now();
            let result=match store.query(&QueryRequest::ActionTimeline(filter)) {
                Ok(answer)=>json!({"case":name,"round":round,"ms":start.elapsed().as_secs_f64()*1000.,"rows":answer.data.as_ref().and_then(|v|v.as_array()).map(Vec::len),"has_more":answer.page.as_ref().map(|p|p.has_more),"status":answer.status}),
                Err(error)=>json!({"case":name,"round":round,"ms":start.elapsed().as_secs_f64()*1000.,"error":error.to_string()}),
            };
            println!("{result}");metrics.push(result);
        }
    }
    std::fs::write(&args[2],serde_json::to_vec_pretty(&json!({"read_only":true,"budget_ms":6000,"metrics":metrics}))?)?;
    ensure!(metrics.iter().all(|r|r.get("error").is_none()),"one or more filtered queries failed");
    Ok(())
}
