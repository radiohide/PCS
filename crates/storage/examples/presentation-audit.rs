//! Read-only inspection of the product-facing names on retained data; never migrates a database.
use anyhow::{Context, Result};
use pcscope_contracts::query::{ActionFilter, InstanceFilter, InstanceSort, QueryRequest};
use serde_json::json;
use std::time::{Duration, Instant};

fn main() -> Result<()> {
    let database = std::env::args().nth(1).context("presentation-audit <database>")?;
    let mut reports = Vec::new();
    for (name, request) in [
        ("software", QueryRequest::InstanceList(InstanceFilter { type_id: Some("software.installation_registration".into()), sort: InstanceSort::NameAsc, limit: 200, ..Default::default() })),
        ("timeline", QueryRequest::ActionTimeline(ActionFilter { limit: 80, ..Default::default() })),
        ("task_history", QueryRequest::ActionTimeline(ActionFilter { action_id:Some("startup.task_started".into()),limit:40,..Default::default() })),
    ] {
        let store = pcscope_storage::Store::open_reader(&database, Duration::from_secs(6))?;
        store.reader().execute_batch("BEGIN;")?;
        let start = Instant::now();
        let response = store.query(&request)?;
        let elapsed=start.elapsed().as_millis();
        let mut report=json!({"query":name,"elapsed_ms":elapsed,"response":response});
        if name=="software" {
            let rows=response.data.as_ref().and_then(serde_json::Value::as_array).context("software rows")?;
            let mut lookups=Vec::new();
            for row in rows.iter().filter(|row|row["presentation"]["name_status"]=="dictionary_matched") {
                let readable=row["presentation"]["name"].as_str().context("dictionary name")?;
                let start=Instant::now();
                let found=store.query(&QueryRequest::InstanceList(InstanceFilter{type_id:Some("software.installation_registration".into()),search:Some(readable.into()),..Default::default()}))?;
                let matching=found.data.as_ref().and_then(serde_json::Value::as_array).is_some_and(|matches|matches.iter().any(|candidate|candidate["instance_id"]==row["instance_id"]));
                anyhow::ensure!(matching,"dictionary name search did not return its original registration");
                lookups.push(json!({"name":readable,"elapsed_ms":start.elapsed().as_millis(),"matched":matching}));
            }
            let start=Instant::now();let mut cursor=None;let mut seen=Vec::new();let mut page_times=Vec::new();
            loop {
                let page_start=Instant::now();
                let page=store.query(&QueryRequest::InstanceList(InstanceFilter{type_id:Some("software.installation_registration".into()),sort:InstanceSort::NameAsc,limit:7,cursor,..Default::default()}))?;
                page_times.push(page_start.elapsed().as_millis());
                seen.extend(page.data.as_ref().and_then(serde_json::Value::as_array).context("page rows")?.iter().map(|row|row["instance_id"].clone()));
                cursor=page.page.context("page metadata")?.next_cursor;if cursor.is_none(){break}
            }
            let expected:Vec<_>=rows.iter().map(|row|row["instance_id"].clone()).collect();
            anyhow::ensure!(seen==expected,"software name pagination disagrees with the full ordered result");
            let count=|state:&str|rows.iter().filter(|row|row["presentation"]["name_status"]==state).count();
            report["verification"]=json!({"source_provided":count("source_provided"),"dictionary_matched":count("dictionary_matched"),"not_provided":count("not_provided"),"name_searches":lookups,"pagination":{"rows":seen.len(),"same_order":true,"elapsed_ms":start.elapsed().as_millis(),"page_elapsed_ms":page_times}});
        }
        store.reader().execute_batch("ROLLBACK;")?;
        reports.push(report);
    }
    println!("{}", serde_json::to_string_pretty(&reports)?);
    Ok(())
}
