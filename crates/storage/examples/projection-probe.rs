//! Read one queued shell snapshot through a read-only source connection; replay only in memory.
use anyhow::{Context, Result};
use pcscope_storage::Store;
use rusqlite::{Connection, OpenFlags};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

fn main() -> Result<()> {
    let source=std::env::args().nth(1).context("projection-probe <read-only-source-db>")?;
    let conn=Connection::open_with_flags(source,OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    conn.busy_timeout(Duration::from_secs(2))?;
    println!("{}",json!({"phase":"read_only_open","sqlite":conn.query_row("SELECT sqlite_version()",[],|r|r.get::<_,String>(0))?}));
    let sql="SELECT e.payload_ref FROM
        (SELECT queue_id,evidence_id,item_id FROM projection_queue WHERE projection='object_model'
          AND state='pending' ORDER BY queue_id LIMIT 64) q CROSS JOIN evidence e INDEXED BY sqlite_autoindex_evidence_1
        ON e.evidence_id=q.evidence_id AND e.item_id=q.item_id
        WHERE e.kind='snapshot' AND e.source='pcscope.shell.registrations' ORDER BY q.queue_id LIMIT 1";
    let plan=conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?.query_map([],|r|r.get::<_,String>(3))?.collect::<rusqlite::Result<Vec<_>>>()?;
    println!("{}",json!({"phase":"input_plan","plan":plan}));
    if std::env::args().any(|arg|arg=="--plan"){
        for statement in ["SELECT count(*) FROM projection_queue WHERE state='pending'",
            "SELECT DISTINCT b.instance_id FROM source_bindings b WHERE b.source=?1 AND b.item_id=?2 AND b.native_record_id IN (?3,?4) AND (b.scope IN (?5,?6) OR ?7='') AND (EXISTS(SELECT 1 FROM evidence_refs r WHERE r.subject_kind='observation' AND r.subject_id=CAST(b.instance_id AS TEXT) AND r.evidence_id=?4) OR EXISTS(SELECT 1 FROM object_actions a JOIN evidence_refs r ON r.subject_kind='action' AND r.subject_id=CAST(a.action_row AS TEXT) WHERE a.instance_id=b.instance_id AND r.evidence_id=?4))"] {
            let mut query=conn.prepare(&format!("EXPLAIN QUERY PLAN {statement}"))?;
            let args=vec!["";query.parameter_count()];
            let plan=query.query_map(rusqlite::params_from_iter(args),|r|r.get::<_,String>(3))?.collect::<rusqlite::Result<Vec<_>>>()?;
            println!("{}",json!({"phase":"maintenance_plan","plan":plan}));
        }
        return Ok(());
    }
    let reference:String=conn.query_row(sql,[],|r|r.get(0))?;
    println!("{}",json!({"phase":"snapshot_selected"}));
    let keys:Vec<_>=reference.strip_prefix("snapshot:").context("not a snapshot")?.split('|').take(3).collect();
    let text:String=conn.query_row("SELECT payload FROM snapshots WHERE source=? AND scope=? AND run_id=?",rusqlite::params![keys[0],keys[1],keys[2]],|r|r.get(0))?;
    drop(conn);
    let mut payload:Value=serde_json::from_str(&text)?;
    payload["protocol_version"]=json!(1);payload["request_id"]=json!("isolated-projection-probe");
    let items=payload["items"].as_array().context("missing items")?.len();
    let envelope=pcscope_contracts::parse(&payload.to_string())?;
    println!("{}",json!({"phase":"isolated_input","items":items,"bytes":text.len()}));
    let mut store=Store::open(":memory:")?;
    let start=Instant::now();store.ingest(&envelope)?;
    println!("{}",json!({"phase":"isolated_ingest","items":items,"bytes":text.len(),"elapsed_ms":start.elapsed().as_millis()}));
    let start=Instant::now();let mut total=0;
    while start.elapsed()<Duration::from_secs(20) {
        let slice=Instant::now();let report=store.project_pending("probe","probe",64)?;total+=report.examined;
        println!("{}",json!({"phase":"isolated_projection","examined":report.examined,"projected":report.projected,"rejected":report.rejected,"total":total,"elapsed_ms":slice.elapsed().as_millis()}));
        if report.examined==0 {break;}
    }
    Ok(())
}
