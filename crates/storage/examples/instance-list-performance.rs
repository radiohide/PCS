//! Read-only latency/equivalence probe; each request has the production six-second budget.
use anyhow::{Context, Result};
use pcscope_contracts::query::{InstanceFilter, InstanceSort, QueryRequest};
use serde_json::json;
use std::time::{Duration, Instant};

fn main() -> Result<()> {
    let database = std::env::args().nth(1).context("instance-list-performance <database> [sort] [pages] [category]")?;
    let sort = match std::env::args().nth(2).as_deref().unwrap_or("name_asc") {
        "name_asc" => InstanceSort::NameAsc,
        "name_desc" => InstanceSort::NameDesc,
        "changed_desc" => InstanceSort::ChangedDesc,
        "seen_desc" => InstanceSort::SeenDesc,
        "type_asc" => InstanceSort::TypeAsc,
        other => anyhow::bail!("unsupported sort {other}"),
    };
    let pages = std::env::args().nth(3).map(|value| value.parse::<usize>()).transpose()?.unwrap_or(1).clamp(1, 10);
    let category = std::env::args().nth(4);
    let mut reports = Vec::new();
    let mut cursor = None;
    let mut passed = true;
    for page_index in 0..pages {
        let store = pcscope_storage::Store::open_reader(&database, Duration::from_secs(6))?;
        let start = Instant::now();
        match store.query(&QueryRequest::InstanceList(InstanceFilter { sort, category: category.clone(), limit: 50, cursor, ..Default::default() })) {
            Ok(response) => {
                cursor = response.page.as_ref().and_then(|page| page.next_cursor.clone());
                reports.push(json!({"page_index":page_index,"sort":sort,"category":category,"elapsed_ms":start.elapsed().as_millis(),"rows":response.data.as_ref().and_then(|v|v.as_array()).map(Vec::len),"counts":response.counts,"page":response.page}));
                if cursor.is_none() { break; }
            }
            Err(error) => {
                reports.push(json!({"page_index":page_index,"sort":sort,"category":category,"elapsed_ms":start.elapsed().as_millis(),"error":error.to_string()}));
                passed = false;
                break;
            }
        }
    }
    println!("{}", serde_json::to_string_pretty(&reports)?);
    anyhow::ensure!(passed, "the query failed; see the JSON report");
    Ok(())
}
