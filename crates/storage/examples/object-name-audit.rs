//! One-shot, read-only audit of naming query results, with the production per-page time budget.
use anyhow::{Context,Result};
use pcscope_contracts::query::{InstanceFilter,InstanceSort,QueryRequest,LifecycleFilter};
use std::time::{Duration,Instant};
fn main()->Result<()> {
    let database=std::env::args().nth(1).context("object-name-audit <database> <output> <type>...")?;
    let output=std::env::args().nth(2).context("output is required")?;
    let mut rows=Vec::new();let mut timings=Vec::new();
    for kind in std::env::args().skip(3) {
        let mut cursor=None;
        for _ in 0..200 {
            let store=pcscope_storage::Store::open_reader(&database,Duration::from_secs(6))?;
            let start=Instant::now();
            let response=store.query(&QueryRequest::InstanceList(InstanceFilter{type_id:Some(kind.clone()),lifecycle:Some(LifecycleFilter::Any),sort:InstanceSort::TypeAsc,limit:200,cursor,..Default::default()}))?;
            timings.push(start.elapsed().as_millis());
            rows.extend(response.data.context("missing response data")?.as_array().context("expected rows")?.clone());
            cursor=response.page.context("missing page")?.next_cursor;
            if cursor.is_none(){break;}
        }
        anyhow::ensure!(cursor.is_none(),"audit page budget exceeded");
    }
    // Match the actual IPC boundary: do not audit raw decrypted-looking values as UI names.
    let count=rows.len();
    let store=pcscope_storage::Store::open_reader(&database,Duration::from_secs(6))?;
    let mut visible=serde_json::json!(rows);
    pcscope_contracts::sensitive::present(&mut visible,store.show_sensitive()?);
    std::fs::write(output,serde_json::to_vec(&visible)?)?;
    println!("{}",serde_json::json!({"rows":count,"page_ms":timings}));
    Ok(())
}
