//! Read-only, bounded comparison against one SQLite snapshot. Raw sample stays ignored.
use anyhow::{ensure,Result};
use pcscope_contracts::query::{ActionFilter,QueryRequest};
use pcscope_storage::Store;
use serde_json::json;
use std::time::{Duration,Instant};
fn main()->Result<()>{
    let args:Vec<_>=std::env::args().collect();
    ensure!(args.len()==4,"timeline-load-benchmark <source-db> <metrics-output> <sample-output>");
    let store=Store::open_reader(&args[1],Duration::from_secs(120))?;
    store.reader().execute_batch("BEGIN")?;
    let mut measurements=Vec::new();let mut full=Vec::new();let mut cards=Vec::new();
    for (compact,limit) in [(false,200),(true,200),(true,400),(true,800)]{
        for round in 0..2 {
            let mut cursor=None;let mut times=Vec::new();let mut bytes=0;let mut rows=Vec::new();
            while rows.len()<10000 {
                let start=Instant::now();
                let response=store.query(&QueryRequest::ActionTimeline(ActionFilter{cards_only:compact,limit,cursor,..Default::default()}))?;
                let encoded=serde_json::to_vec(&response)?;
                times.push(start.elapsed().as_secs_f64()*1000.);bytes+=encoded.len();
                rows.extend(response.data.unwrap_or(json!([])).as_array().cloned().unwrap_or_default());
                let page=response.page.unwrap();if !page.has_more{break;}cursor=page.next_cursor;
            }
            measurements.push(json!({"cards_only":compact,"limit":limit,"round":round,"rows":rows.len(),"bytes":bytes,"queries":times.len(),"query_ms":times,"total_ms":times.iter().sum::<f64>()}));
            if round==0 && !compact {full=rows;}
            else if round==0 && compact && limit==200 {cards=rows;}
        }
    }
    ensure!(full.len()==cards.len(),"sample row counts differ");
    for (a,b) in full.iter().zip(&cards) {
        for key in ["action_id","observed_at","instance_id","before","after"] {ensure!(a[key]==b[key],"card ordering or values changed");}
    }
    let detail=store.query(&QueryRequest::ActionDetail{action_row:cards[0]["detail_id"].as_i64().unwrap()})?.data;
    ensure!(detail.as_ref()==full.first(),"first record detail differs");
    store.reader().execute_batch("ROLLBACK")?;
    std::fs::write(&args[2],serde_json::to_vec_pretty(&json!({"read_only":true,"same_snapshot":true,"measurements":measurements}))?)?;
    std::fs::write(&args[3],serde_json::to_vec(&json!({"full":full,"cards":cards}))?)?;
    println!("{}",json!({"metrics":args[2],"sample":args[3],"rows":cards.len(),"read_only":true}));
    Ok(())
}
