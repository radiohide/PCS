//! Isolated real-daemon/real-software-collector smoke test. Never opens the user's database.
use anyhow::{ensure, Result};
use pcscope_ipc::{call, Request};
use pcscope_contracts::query::{QueryRequest, InstanceFilter};
use pcscope_storage::Store;
use serde_json::json;
use std::{path::Path, process::{Child, Command}, time::{Duration, Instant}};

struct Daemon(Child);
impl Drop for Daemon {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() { let _ = self.0.kill(); let _ = self.0.wait(); }
    }
}
fn launch(exe: &Path, data: &Path) -> Result<Daemon> {
    let mut command = Command::new(exe);
    command.arg(data);
    #[cfg(windows)] {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // Hidden background helper.
    }
    Ok(Daemon(command.spawn()?))
}
async fn await_ready(daemon: &mut Daemon, require_objects: bool) -> Result<u64> {
    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        ensure!(daemon.0.try_wait()?.is_none(), "isolated daemon exited before responding");
        if let Ok(status) = call(Request::Status).await {
            if status["projection_worker"]["state"] == "idle" && status["collecting"] == false {
                let result = call(Request::Query {request: QueryRequest::InstanceList(InstanceFilter::default())}).await?;
                let count = result["counts"]["instances"].as_u64().unwrap_or(0);
                if !require_objects || count > 0 { return Ok(count); }
            }
        }
        ensure!(Instant::now() < deadline, "runtime projection did not produce queryable objects");
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}
async fn stop(daemon: &mut Daemon) -> Result<()> {
    call(Request::Shutdown).await?;
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = daemon.0.try_wait()? {ensure!(status.success(), "daemon shutdown failed");return Ok(());}
        ensure!(Instant::now() < deadline, "daemon shutdown timeout");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
#[tokio::main]
async fn main() -> Result<()> {
    ensure!(call(Request::Status).await.is_err(), "another daemon is running; leave it untouched");
    let root = std::env::current_exe()?.parent().unwrap().parent().unwrap().to_owned();
    let data = std::env::current_dir()?.join("data").join(format!("s32-runtime-{}",chrono::Utc::now().timestamp_nanos_opt().unwrap()));
    std::fs::create_dir_all(&data)?;
    let mut settings = json!({});
    for id in ["pcscope-software.exe","pcscope-powershell-adapter.exe","system","security","network","storage","startup","desktop","virtualization","applications","scoped","eventlog","usn"] {
        settings[id] = json!({"enabled":id=="pcscope-software.exe","seconds":604800});
    }
    Store::open(data.join("history.db"))?.save_plugin_settings(&settings)?;
    let mut daemon = launch(&root.join("pcscope-service.exe"), &data)?;
    let count = await_ready(&mut daemon, true).await?;
    let projection = call(Request::Query {request:QueryRequest::ProjectionStatus}).await?;
    println!("PASS real collector -> daemon worker -> model query: instances={count}; projection={}",projection["data"]);
    stop(&mut daemon).await?;
    let identity;
    {
        let store = Store::open(data.join("history.db"))?;
        identity = store.reader().query_row("SELECT value FROM settings WHERE key='projection_installation'", [], |r|r.get::<_,String>(0))?;
        settings["pcscope-software.exe"]["enabled"] = json!(false);
        store.save_plugin_settings(&settings)?;
    }
    let mut daemon = launch(&root.join("pcscope-service.exe"), &data)?;
    let after = await_ready(&mut daemon, true).await?;
    ensure!(after==count,"restart changed instance count");
    let live = call(Request::Query {request:QueryRequest::ProjectionStatus}).await?;
    ensure!(live["data"].as_array().unwrap().iter().all(|row|row["pending"]==0),"pending work remains while idle after restart");
    stop(&mut daemon).await?;
    let store = Store::open(data.join("history.db"))?;
    let saved: String = store.reader().query_row("SELECT value FROM settings WHERE key='projection_installation'", [], |r|r.get(0))?;
    ensure!(identity==saved,"restart changed installation identity");
    let pending: i64 = store.reader().query_row("SELECT count(*) FROM projection_queue WHERE state='pending'", [], |r|r.get(0))?;
    // end_session itself records evidence after the worker stops. It is durable next-start work.
    println!("PASS restart: instances={after}, live pending=0, shutdown pending={pending}; isolated artifacts={}",data.display());
    Ok(())
}
