//! Read-only extraction of a bounded real-history sample; all writes go to a NEW isolated DB.
//! Outputs aggregate timings only. Never commit the sample database or raw records.
use anyhow::{ensure,Result};
use pcscope_contracts::query::{ActionFilter,InstanceFilter,QueryRequest};
use pcscope_storage::Store;
use rusqlite::{params,Connection};
use serde_json::{json,Value};
use std::time::{Duration,Instant};

fn copy(conn:&Connection,table:&str,condition:&str)->Result<()> {
    let columns={let mut q=conn.prepare(&format!("PRAGMA main.table_info({table})"))?;
        let rows=q.query_map([],|r|r.get::<_,String>(1))?.collect::<rusqlite::Result<Vec<_>>>()?;rows.join(",")};
    conn.execute_batch(&format!("INSERT OR IGNORE INTO main.{table}({columns}) SELECT {columns} FROM live.{table} WHERE {condition};"))?;
    Ok(())
}
fn measure(store:&Store,request:&QueryRequest)->Result<(Value,Value)> {
    let mut times=Vec::new();let mut response=Value::Null;
    for _ in 0..5 {let start=Instant::now();response=serde_json::to_value(store.query(request)?)?;times.push(start.elapsed().as_secs_f64()*1000.);}
    times.sort_by(f64::total_cmp);
    let metrics=json!({"median_ms":times[2],"max_ms":times[4],"rows":response["data"].as_array().map(Vec::len),"bytes":response.to_string().len()});
    Ok((metrics,response))
}
fn main()->Result<()> {
    let args:Vec<_>=std::env::args().collect();ensure!(args.len()==3,"display-benchmark <read-only-source-db|--existing-sample> <sample-db>");
    let existing=args[1]=="--existing-sample";
    if existing {
        let reader=Store::open_reader(&args[2],Duration::from_secs(6))?;
        let marked:bool=reader.reader().query_row("SELECT EXISTS(SELECT 1 FROM settings WHERE key='display_benchmark_sample' AND value='v1')",[],|r|r.get(0))?;
        ensure!(marked,"refusing to write: database is not a marked disposable benchmark sample");
    }else{ensure!(!std::path::Path::new(&args[2]).exists(),"sample path already exists; refusing to overwrite");}
    let store=Store::open(&args[2])?;let conn=store.reader();
    let extract=Instant::now();
    if !existing {
    let uri=format!("file:{}?mode=ro",args[1].replace('\\',"/"));
    conn.execute("ATTACH DATABASE ? AS live",[uri])?;
    let tx=conn.unchecked_transaction()?;
    tx.execute_batch("CREATE TEMP TABLE picked_actions AS SELECT action_row FROM live.object_actions ORDER BY action_row DESC LIMIT 2000;
        CREATE TEMP TABLE picked_instances AS SELECT instance_id FROM live.object_instances ORDER BY instance_id LIMIT 200;
        INSERT INTO picked_instances SELECT DISTINCT instance_id FROM live.object_actions WHERE action_row IN picked_actions AND instance_id IS NOT NULL;")?;
    copy(&tx,"object_instances","instance_id IN picked_instances")?;
    copy(&tx,"object_facts","instance_id IN picked_instances")?;
    copy(&tx,"fact_history","instance_id IN picked_instances")?;
    copy(&tx,"source_bindings","instance_id IN picked_instances")?;
    copy(&tx,"object_actions","action_row IN picked_actions")?;
    copy(&tx,"evidence_refs","subject_kind='action' AND subject_id IN (SELECT CAST(action_row AS TEXT) FROM picked_actions)")?;
    let ids={let mut q=tx.prepare("SELECT DISTINCT instance_id FROM picked_instances")?;
        let rows=q.query_map([],|r|r.get::<_,i64>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;rows};
    for id in &ids {
        tx.execute("INSERT OR IGNORE INTO main.evidence_refs SELECT * FROM live.evidence_refs WHERE subject_kind='observation' AND subject_id=? LIMIT 2",[id.to_string()])?;
    }
    // The address points to one whole snapshot; copy metadata only for selected native items.
    copy(&tx,"evidence","evidence_id IN (SELECT evidence_id FROM main.evidence_refs) AND (kind='event' OR EXISTS(SELECT 1 FROM main.source_bindings b WHERE b.source=evidence.source AND b.item_id=evidence.item_id))")?;
    copy(&tx,"records","seq IN (SELECT CAST(substr(payload_ref,8) AS INTEGER) FROM main.evidence WHERE payload_ref LIKE 'record:%')")?;
    copy(&tx,"objects","EXISTS(SELECT 1 FROM main.source_bindings b WHERE b.source=objects.source AND b.item_id=objects.id)")?;
    copy(&tx,"coverage_scopes","source IN (SELECT source FROM main.source_bindings)")?;
    let snapshots={let mut q=tx.prepare("SELECT DISTINCT payload_ref FROM main.evidence WHERE kind='snapshot'")?;
        let rows=q.query_map([],|r|r.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;rows};
    for reference in snapshots {
        let parts:Vec<_>=reference.trim_start_matches("snapshot:").splitn(4,'|').collect();
        if parts.len()==4 {tx.execute("INSERT OR IGNORE INTO main.snapshots SELECT * FROM live.snapshots WHERE source=? AND scope=? AND run_id=?",params![parts[0],parts[1],parts[2]])?;}
    }
    tx.execute_batch("UPDATE display_build SET object_bound=(SELECT coalesce(max(instance_id),0) FROM object_instances),action_bound=(SELECT coalesce(max(action_row),0) FROM object_actions),evidence_bound=(SELECT coalesce(max(seq),0) FROM evidence);")?;
    tx.execute("INSERT INTO settings VALUES('display_benchmark_sample','v1')",[])?;
    tx.commit()?;conn.execute_batch("DETACH DATABASE live;")?;
    }else{
        conn.execute_batch("DELETE FROM display_objects;DELETE FROM display_actions;
            UPDATE display_totals SET initialised=0,addresses_done=0,address_cursor='';
            UPDATE display_build SET object_cursor=0,action_cursor=0,evidence_cursor=0,published=0;
            INSERT OR IGNORE INTO display_work(kind,id) SELECT 'object',instance_id FROM object_instances;
            INSERT OR IGNORE INTO display_work(kind,id) SELECT 'action',action_row FROM object_actions;")?;
    }
    let extract_ms=extract.elapsed().as_secs_f64()*1000.;
    let objects=QueryRequest::InstanceList(InstanceFilter{limit:200,..Default::default()});
    let actions=QueryRequest::ActionTimeline(ActionFilter{limit:200,..Default::default()});
    let (cold_objects,expected_objects)=measure(&store,&objects)?;let (cold_actions,expected_actions)=measure(&store,&actions)?;
    let start=Instant::now();let mut slices=0;let mut max_slice_ms=0f64;
    loop {
        let report=store.prepare_display(32,Duration::from_millis(40))?;slices+=1;
        max_slice_ms=max_slice_ms.max(report["elapsed_ms"].as_f64().unwrap_or_default());
        ensure!(report["failed"]==0,"sample preparation failed (see isolated DB display_work; raw values not printed)");
        if report["pending"]==false {break;}
        ensure!(slices<10000,"bounded sample failed to converge");
    }
    let build_ms=start.elapsed().as_secs_f64()*1000.;
    let (warm_objects,actual_objects)=measure(&store,&objects)?;let(warm_actions,actual_actions)=measure(&store,&actions)?;
    // Scope recovery deliberately corrects the empty cold coverage; all other conclusions match.
    let normalize=|mut response:Value,objects:bool| {
        if objects {if let Some(rows)=response["data"].as_array_mut(){for row in rows {row["coverage"]=json!([]);}}}
        response["page"]["watermark"]=Value::Null;
        response["page"]["next_cursor"]=json!(response["page"]["next_cursor"].is_string());response
    };
    ensure!(normalize(expected_objects,true)==normalize(actual_objects,true),"object results changed beyond precise scope recovery");
    ensure!(normalize(expected_actions,false)==normalize(actual_actions,false),"timeline results changed when using precomputed views");
    let count=|table:&str|->Result<i64>{Ok(conn.query_row(&format!("SELECT count(*) FROM {table}"),[],|r|r.get(0))?)};
    println!("{}",serde_json::to_string_pretty(&json!({"basis":"bounded real-history sample in isolated database; five runs, same queries; no IPC or UI time",
        "sample":{"objects":count("object_instances")?,"actions":count("object_actions")?,"evidence_metadata":count("evidence")?,"raw_records":count("records")?,"snapshots":count("snapshots")?},
        "extract_ms":extract_ms,"existing_sample":existing,
        "cold":{"objects":cold_objects,"timeline":cold_actions},"precomputed":{"objects":warm_objects,"timeline":warm_actions},
        "preparation":{"slices":slices,"total_ms":build_ms,"max_slice_ms":max_slice_ms},"semantic_match":true,
        "sample_db_bytes":std::fs::metadata(&args[2])?.len()}))?);
    Ok(())
}
