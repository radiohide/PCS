//! Full migrated-store COPY only. No production database or production daemon is touched.
use anyhow::{ensure, Context, Result};
use pcscope_ipc::{call, Request};
use pcscope_contracts::query::QueryRequest;
use pcscope_storage::Store;
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde_json::{json, Value};
use std::{path::{Path, PathBuf}, process::{Child, Command}, time::{Duration, Instant}};

fn reader(path: &Path) -> Result<Connection> {
    Ok(Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?)
}
fn counts(conn: &Connection) -> Result<Value> {
    let tx = conn.unchecked_transaction()?;
    let conn = &tx;
    let mut value = json!({});
    for table in ["records","snapshots","evidence","object_instances","evidence_refs"] {
        value[table] = json!(conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |r|r.get::<_,i64>(0))?);
    }
    for state in ["pending","done","failed"] {
        value[state] = json!(conn.query_row("SELECT count(*) FROM projection_queue WHERE projection=? AND state=?",
            [pcscope_storage::evidence::OBJECT_PROJECTION, state], |r|r.get::<_,i64>(0))?);
    }
    Ok(value)
}
fn settings() -> Value {
    let mut settings = json!({});
    for id in ["pcscope-software.exe","pcscope-powershell-adapter.exe","system","security","network","storage","startup","desktop","virtualization","applications","scoped","eventlog","usn"] {
        settings[id] = json!({"enabled":false,"seconds":604800});
    }
    settings
}
fn completeness(conn: &Connection) -> Result<Value> {
    let scalar = |sql: &str| conn.query_row(sql, [], |r|r.get::<_,i64>(0));
    Ok(json!({
        "legacy_eventlog_records":scalar("SELECT count(*) FROM records WHERE source LIKE 'pcscope.eventlog.%'")?,
        "eventlog_evidence":scalar("SELECT count(*) FROM evidence WHERE source LIKE 'pcscope.eventlog.%'")?,
        "evidence_sources":scalar("SELECT count(DISTINCT source) FROM evidence")?,
        "coverage_sources":scalar("SELECT count(DISTINCT source) FROM coverage_scopes")?,
        "sources_without_coverage":scalar("SELECT count(*) FROM (SELECT DISTINCT source FROM evidence EXCEPT SELECT DISTINCT source FROM coverage_scopes)")?
    }))
}
fn prepare() -> Result<()> {
    let root = std::env::current_dir()?.canonicalize()?;
    let source = root.join("data/s32-rehearsal/history.db");
    let source = reader(&source)?;
    ensure!(source.query_row("PRAGMA user_version", [], |r|r.get::<_,i64>(0))? == 8, "expected migrated rehearsal schema 8");
    let before = counts(&source)?;
    ensure!(before["object_instances"]==0, "source is already projected; preserve it");
    let data = root.join("data").join(format!("s32-scale-{}",chrono::Utc::now().timestamp_nanos_opt().unwrap()));
    std::fs::create_dir(&data)?;
    let mut destination = Connection::open(data.join("history.db"))?;
    let started = Instant::now();
    let backup = rusqlite::backup::Backup::new(&source, &mut destination)?;
    backup.run_to_completion(8192, Duration::from_millis(1), None)?;
    drop(backup);
    ensure!(counts(&destination)?==before,"backup count mismatch");
    let integrity: String = destination.query_row("PRAGMA integrity_check", [], |r|r.get(0))?;
    ensure!(integrity=="ok", "backup integrity check failed");
    drop(destination);
    let mut store = Store::open(data.join("history.db"))?;
    store.save_plugin_settings(&settings())?;
    store.set_monitoring_settings(&json!({"mode":"low","repair":false,"configured":true}),&chrono::Utc::now().to_rfc3339())?;
    println!("{}",json!({"prepared":data,"seconds":started.elapsed().as_secs_f64(),"counts":before,"integrity":integrity}));
    Ok(())
}
struct Daemon(Child);
impl Drop for Daemon {
    fn drop(&mut self) {if self.0.try_wait().ok().flatten().is_none(){let _=self.0.kill();let _=self.0.wait();}}
}
fn launch(data: &Path) -> Result<Daemon> {
    let executable = std::env::current_exe()?.parent().unwrap().parent().unwrap().join("pcscope-service.exe");
    let mut command = Command::new(executable);
    command.arg(data);
    #[cfg(windows)] {use std::os::windows::process::CommandExt;command.creation_flags(0x08000000);}
    Ok(Daemon(command.spawn()?))
}
#[tokio::main]
async fn main() -> Result<()> {
    ensure!(call(Request::Status).await.is_err(), "another daemon is running; leave it untouched");
    let mode = std::env::args().nth(1).context("usage: runtime-projection-scale prepare | run <isolated-data-dir> <seconds> [crash]")?;
    if mode=="prepare" {return prepare();}
    ensure!(mode=="run" || mode=="audit","unknown mode");
    let data = PathBuf::from(std::env::args().nth(2).context("data directory required")?).canonicalize()?;
    let parent = std::env::current_dir()?.join("data").canonicalize()?;
    ensure!(data.parent()==Some(parent.as_path()) && data.file_name().unwrap().to_string_lossy().starts_with("s32-scale-"),
        "only a named isolated scale-copy directory is allowed");
    let seconds: u64 = std::env::args().nth(3).unwrap_or_else(||"60".into()).parse()?;
    let crash = std::env::args().nth(4).as_deref()==Some("crash");
    let db = data.join("history.db");
    if mode=="audit" {
        let conn = reader(&db)?;
        println!("{}",json!({"counts":counts(&conn)?,"completeness":completeness(&conn)?}));
        return Ok(());
    }
    let monitor: Option<String> = reader(&db)?.query_row("SELECT value FROM settings WHERE key='monitoring'", [], |r|r.get(0)).optional()?;
    if let Some(monitor) = monitor {
        let monitor: Value = serde_json::from_str(&monitor)?;
        ensure!(monitor["repair"]!=true,"test copy must not enable machine audit repair");
    }
    let before = counts(&reader(&db)?)?;
    let identity: Option<String> = reader(&db)?.query_row("SELECT value FROM settings WHERE key='projection_installation'", [], |r|r.get(0)).optional()?;
    println!("{}",json!({"before":before,"data":data}));
    let mut daemon = launch(&data)?;
    let started = Instant::now();
    let mut drained = false;
    while started.elapsed() < Duration::from_secs(seconds) {
        tokio::time::sleep(Duration::from_secs(10)).await;
        ensure!(daemon.0.try_wait()?.is_none(),"test daemon exited early");
        let snapshot = counts(&reader(&db)?)?;
        let query_started = Instant::now();
        let query = call(Request::Query {request:QueryRequest::ProjectionStatus}).await;
        let query_ms = query_started.elapsed().as_millis();
        let status = match query {Ok(_)=>"ok".to_owned(),Err(e)=>e.to_string()};
        println!("{}",json!({"seconds":started.elapsed().as_secs(),"counts":snapshot,"projection_query_ms":query_ms,"query":status}));
        if snapshot["pending"]==0 {drained=true;break;}
    }
    if crash {
        daemon.0.kill()?;daemon.0.wait()?;
    } else {
        call(Request::Shutdown).await?;
        let stopping = Instant::now();
        while daemon.0.try_wait()?.is_none() {
            ensure!(stopping.elapsed()<Duration::from_secs(45),"graceful shutdown timeout");
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
    let conn = reader(&db)?;
    let after = counts(&conn)?;
    let saved: String = conn.query_row("SELECT value FROM settings WHERE key='projection_installation'", [], |r|r.get(0))?;
    if let Some(identity) = identity {ensure!(identity==saved,"restart changed projection identity");}
    ensure!(after["evidence"].as_i64()>=before["evidence"].as_i64(),"evidence was lost");
    ensure!(after["snapshots"]==before["snapshots"],"raw snapshots changed");
    let mut errors = conn.prepare("SELECT e.source,count(*) FROM projection_queue q JOIN evidence e ON e.evidence_id=q.evidence_id AND e.item_id=q.item_id WHERE q.state='failed' GROUP BY e.source ORDER BY count(*) DESC LIMIT 12")?;
    let failed = errors.query_map([],|r|Ok(json!({"source":r.get::<_,String>(0)?,"count":r.get::<_,i64>(1)?})))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    println!("{}",json!({"failed_sources":failed}));
    println!("{}",json!({"completeness":completeness(&conn)?}));
    println!("{}",json!({"after":after,"drained":drained,"crash":crash,"seconds":started.elapsed().as_secs()}));
    Ok(())
}
