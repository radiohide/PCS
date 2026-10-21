//! End-to-end daemon smoke probe. Run against a live development daemon.
//!
//! The read-side modes that used the legacy per-view operations (`self-activity`, `key-events`,
//! `collection-view`, `objects`) were removed on 2026-09-12 along with the operations themselves —
//! see docs/full-object-refactor-plan.md §19. What they asserted now belongs to the object model:
//! self-activity classification is applied at ingest (`storage/src/self_activity.rs`, covered by
//! unit tests), and the view shapes are the query contract's business (`crates/contracts`, plus the
//! `scripts/check-*.cjs` front-end checks). What remains here is what only a live daemon can show:
//! that it starts, collects, answers the query entry point, and shuts down cleanly.

use anyhow::{ensure, Result};
use pcscope_contracts::query::{ActionFilter, InstanceFilter, QueryRequest};
use pcscope_ipc::{call, Request};

/// One object-model query, with the envelope checked rather than only the payload: a response that
/// lost its status or its contract version is not something the frontend can route.
async fn query(request: QueryRequest) -> Result<serde_json::Value> {
    let value = call(Request::Query { request }).await?;
    ensure!(
        value["contract_version"].as_u64().is_some(),
        "query response carries no contract version"
    );
    ensure!(
        value["status"].is_string(),
        "query response carries no status: {value}"
    );
    ensure!(
        value["data"].is_array(),
        "query response carries no data array: {value}"
    );
    Ok(value)
}

/// A paged query must say how to ask for the next page, and whether there is one.
fn paged(value: &serde_json::Value) -> Result<()> {
    ensure!(
        value["page"]["watermark"].is_string(),
        "paged response has no watermark: {value}"
    );
    ensure!(
        value["page"]["has_more"].is_boolean(),
        "paged response has no has_more: {value}"
    );
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::args().nth(1).as_deref()==Some("item-controls") {
        let value=call(Request::ItemControls).await?;let rows=value["items"].as_array().unwrap();
        ensure!(rows.len()==221,"unexpected control count");
        ensure!(rows.iter().any(|r|r["id"]=="pcscope.identity.logon-sessions"&&r["plugin"]=="system"),"missing session control");
        println!("Item controls: {}; disabled: {}; revision: {}",rows.len(),rows.iter().filter(|r|r["enabled"]==false).count(),value["revision"]);
        println!("Retention: {}",call(Request::Retention).await?);return Ok(());
    }
    if std::env::args().nth(1).as_deref()==Some("query-performance") {
        println!("Retention: {}",call(Request::Retention).await?);
        // The reads go through the object model's single entry point, which opens its own reader and
        // takes a query slot. The point of running one of these against a concurrent status call is
        // that a long read must not take the collection writer's lock: both have to answer.
        let query=async {
            let started=std::time::Instant::now();
            let result=query(QueryRequest::ActionTimeline(ActionFilter{limit:100,..Default::default()})).await;
            println!("Full action timeline: {} ms; {}",started.elapsed().as_millis(),match result {Ok(v)=>format!("rows={}",v["data"].as_array().unwrap().len()),Err(e)=>e.to_string()});
        };
        let status=async {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let started=std::time::Instant::now();
            let value=call(Request::Status).await?;
            println!("Concurrent status: {} ms; collecting={}; counts={}",started.elapsed().as_millis(),value["collecting"],value["counts"]);
            Ok::<_,anyhow::Error>(())
        };
        let (_,result)=tokio::join!(query,status);result?;return Ok(());
    }
    if std::env::args().nth(1).as_deref()==Some("file-watch") {
        println!("{}",call(Request::FileWatchSettings).await?);return Ok(());
    }
    if std::env::args().nth(1).as_deref()==Some("monitoring") {
        let result=call(Request::MonitoringSettings).await?;
        ensure!(result["elevated"].is_boolean(),"missing actual privilege");
        ensure!(result["settings"]["configured"].is_boolean(),"missing first-run state");
        if !result["checking"].as_bool().unwrap_or(true) {
            ensure!(result["report"]["logs"].as_array().is_some_and(|logs|logs.len()==23),"capability check failed: {}",result["report"]);
        }
        println!("{}",serde_json::to_string_pretty(&result)?);
        return Ok(());
    }
    if std::env::args().nth(1).as_deref() == Some("plugin-control") {
        let initial = call(Request::Plugins).await?;
        ensure!(
            initial.as_array().is_some_and(|j| j.len() == 13),
            "expected 13 plugins"
        );
        let saved = initial
            .as_array()
            .unwrap()
            .iter()
            .find(|j| j["id"] == "scoped")
            .unwrap()
            .clone();
        call(Request::SetPlugin {
            id: "scoped".into(),
            enabled: Some(false),
            seconds: Some(777),
        })
        .await?;
        let verification: Result<()> = async {
            let configured = call(Request::Plugins).await?;
            let job = configured
                .as_array()
                .unwrap()
                .iter()
                .find(|j| j["id"] == "scoped")
                .unwrap();
            ensure!(
                job["enabled"] == false && job["seconds"] == 777,
                "configuration not applied"
            );
            ensure!(
                call(Request::SetPlugin {
                    id: "scoped".into(),
                    enabled: Some(true),
                    seconds: Some(0)
                })
                .await
                .is_err(),
                "invalid interval accepted"
            );
            ensure!(
                call(Request::CollectPlugin {
                    id: "not-a-plugin".into()
                })
                .await
                .is_err(),
                "unknown plugin accepted"
            );
            let mut accepted = false;
            for _ in 0..180 {
                let status = call(Request::Plugins).await?;
                if status
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|j| j["id"] != "eventlog" && j["id"] != "usn")
                    .all(|j| j["phase"] == "idle")
                {
                    call(Request::CollectPlugin {
                        id: "scoped".into(),
                    })
                    .await?;
                    accepted = true;
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
            ensure!(accepted, "inventory remained busy");
            ensure!(
                call(Request::CollectPlugin {
                    id: "scoped".into()
                })
                .await
                .is_err(),
                "duplicate run accepted"
            );
            let mut finished = false;
            for _ in 0..150 {
                let status = call(Request::Plugins).await?;
                let job = status
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|j| j["id"] == "scoped")
                    .unwrap();
                ensure!(
                    job["enabled"] == false,
                    "manual run enabled automatic collection"
                );
                if job["phase"] == "idle" && job["last_finished"].is_string() {
                    finished = true;
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
            ensure!(finished, "single plugin did not finish");
            Ok(())
        }
        .await;
        let restore = call(Request::SetPlugin {
            id: "scoped".into(),
            enabled: saved["enabled"].as_bool(),
            seconds: saved["seconds"].as_u64(),
        })
        .await;
        verification?;
        restore?;
        println!("PASS: plugin configuration, interval validation, disabled single run, duplicate rejection; original configuration restored");
        return Ok(());
    }
    if std::env::args().nth(1).as_deref() == Some("shutdown") {
        println!("{}", call(Request::Shutdown).await?);
        return Ok(());
    }
    let mut status = call(Request::Status).await?;
    ensure!(
        status["mode"] == "development-user-daemon",
        "unexpected daemon mode"
    );
    call(Request::Collect).await?;
    for _ in 0..1800 {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        status = call(Request::Status).await?;
        if status["collecting"] == false {
            break;
        }
    }
    ensure!(status["collecting"] == false, "collector did not finish");
    ensure!(
        status["usn_collection"].is_object(),
        "USN poll result missing"
    );
    ensure!(
        status["usn_collection"]["error"].is_null(),
        "USN process/ingest failed: {}",
        status["usn_collection"]
    );
    let plugins = status["last_collection"]["plugins"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("plugin results missing"))?;
    ensure!(
        plugins.len() == status["collectors"].as_array().unwrap().len(),
        "not all collector jobs finished"
    );
    ensure!(
        plugins.iter().all(|p| p["error"].is_null()),
        "plugin process/ingest failure: {}",
        status["last_collection"]
    );
    ensure!(
        plugins
            .iter()
            .any(|p| p["ok"] == true && p["changes"].as_u64().unwrap_or(0) > 0),
        "no successful inventory source"
    );
    println!(
        "Coverage: {} collector jobs; {} jobs report explicit source gaps",
        plugins.len(),
        plugins.iter().filter(|p| p["ok"] == false).count()
    );
    // The model's reads. These check the daemon answers the query entry point and returns a
    // well-formed envelope — a daemon that never registered the operation, or that dropped the
    // paging metadata a cursor needs, fails here. What the rows mean is checked by the contract
    // tests and the front-end checks, not by this smoke probe.
    let instances = query(QueryRequest::InstanceList(InstanceFilter {
        limit: 5,
        ..Default::default()
    }))
    .await?;
    paged(&instances)?;
    let actions = query(QueryRequest::ActionTimeline(ActionFilter {
        limit: 5,
        ..Default::default()
    }))
    .await?;
    paged(&actions)?;
    let coverage = query(QueryRequest::SourceCoverage { source: None }).await?;
    let catalog = query(QueryRequest::TypeCatalog).await?;
    ensure!(
        !catalog["data"].as_array().unwrap().is_empty(),
        "the type catalog is empty"
    );
    ensure!(
        catalog["catalog_version"].is_string(),
        "the type catalog carries no version"
    );
    println!(
        "IPC PASS: status, collection, instances ({}), actions ({}), coverage ({}), type catalog; counts={}",
        instances["data"].as_array().unwrap().len(),
        actions["data"].as_array().unwrap().len(),
        coverage["data"].as_array().unwrap().len(),
        status["counts"]
    );
    Ok(())
}
