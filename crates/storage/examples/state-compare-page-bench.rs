//! Read-only, full-branch comparison benchmark; optional watermark permits exact before/after checks.
use anyhow::Result;
use pcscope_contracts::query::QueryRequest;
use pcscope_storage::Store;
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use std::time::{Duration,Instant};
fn main()->Result<()> {
    let args:Vec<String>=std::env::args().collect();
    anyhow::ensure!((5..=6).contains(&args.len()),"state-compare-page-bench <database> <from> <until> <type> [watermark]");
    let mut cursor:Option<String>=None;let mut mark=args.get(5).cloned();let mut objects=Vec::new();let mut pages=Vec::new();let started=Instant::now();
    loop {
        // Production opens a bounded reader per request, so give each page the same budget.
        let store=Store::open_reader(&args[1],Duration::from_secs(6))?;
        let request:QueryRequest=serde_json::from_value(json!({"query":"state_compare","from":args[2],"until":args[3],"type_id":args[4],"watermark":mark,"cursor":cursor,"limit":50}))?;
        let begin=Instant::now();let response=store.query(&request)?;let query_ms=begin.elapsed().as_millis();
        let bytes=serde_json::to_vec(&response)?.len();let rows=response.data.as_ref().unwrap()["objects"].as_array().unwrap();
        pages.push(json!({"query_ms":query_ms,"bytes":bytes,"rows":rows.len()}));objects.extend(rows.iter().cloned());
        let page=response.page.unwrap();mark=Some(page.watermark);if !page.has_more {break}
        anyhow::ensure!(page.next_cursor.is_some()&&page.next_cursor!=cursor,"cursor made no progress");cursor=page.next_cursor;
    }
    let elapsed=started.elapsed().as_millis();let digest=format!("{:x}",Sha256::digest(serde_json::to_vec(&objects)?));
    println!("{}",json!({"elapsed_ms":elapsed,"object_count":objects.len(),"objects_sha256":digest,"pages":Value::Array(pages),"watermark":mark}));Ok(())
}
