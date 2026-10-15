//! Read-only aggregate audit of retained changes. No object names, values or private material.
use anyhow::{Context,Result};
use pcscope_contracts::query::{ActionFilter,QueryRequest};
use serde_json::json;
use std::{collections::BTreeMap,time::{Duration,Instant}};

fn main()->Result<()> {
    let database=std::env::args().nth(1).context("timeline-policy-audit <database>")?;
    let store=pcscope_storage::Store::open_reader(database,Duration::from_secs(30))?;
    store.reader().execute_batch("BEGIN;")?;
    let start=Instant::now();let mut reports=Vec::new();
    let count=||store.reader().query_row("SELECT count(*) FROM object_actions",[],|r|r.get::<_,i64>(0));
    let before=count()?;
    for routine in [true,false] {
        let mut counts=BTreeMap::<String,usize>::new();let mut pages=0;
        for id in ["observed.state_changed","observed.configuration_changed"] {
            let mut cursor=None;
            loop {
                let response=store.query(&QueryRequest::ActionTimeline(ActionFilter{action_id:Some(id.into()),include_routine:routine,cursor,limit:200,..Default::default()}))?;
                for row in response.data.context("missing query data")?.as_array().context("expected actions")? {
                    let key=format!("{}.{}",row["context"]["type_id"].as_str().unwrap_or("unassigned"),row["field_id"].as_str().unwrap_or(""));
                    *counts.entry(key).or_default()+=1;
                }
                pages+=1;cursor=response.page.context("missing page")?.next_cursor;
                if cursor.is_none(){break}
                anyhow::ensure!(pages<200,"audit page limit reached");
            }
        }
        reports.push(json!({"include_routine":routine,"changes":counts.values().sum::<usize>(),"fields":counts,"pages":pages}));
    }
    anyhow::ensure!(count()?==before,"read snapshot changed unexpectedly");
    store.reader().execute_batch("ROLLBACK;")?;
    println!("{}",serde_json::to_string_pretty(&json!({"read_only":true,"actions_retained":before,"elapsed_ms":start.elapsed().as_millis(),"views":reports}))?);
    Ok(())
}
