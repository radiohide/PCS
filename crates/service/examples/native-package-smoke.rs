//! Real packaged native collectors -> isolated daemon -> the same object query used by the UI.
//! No device/network changes, formal endpoint, elevated token or existing database.
use anyhow::{ensure,Result};
use pcscope_ipc::{call_on,Endpoint,Request};
use pcscope_contracts::query::{InstanceFilter,QueryRequest};
use pcscope_storage::Store;
use serde_json::json;
use std::{process::{Child,Command,Stdio},time::{Duration,Instant}};
struct Daemon(Child);
impl Drop for Daemon {fn drop(&mut self){if self.0.try_wait().ok().flatten().is_none(){let _=self.0.kill();let _=self.0.wait();}}}
async fn count(endpoint:&Endpoint,kind:&str)->Result<u64>{
    let filter=InstanceFilter{type_id:Some(kind.into()),..Default::default()};
    let result=call_on(endpoint,Request::Query{request:QueryRequest::InstanceList(filter)}).await?;
    Ok(result["counts"]["instances"].as_u64().unwrap_or(0))
}
fn ordinary_token()->Result<()> {
    use windows_sys::Win32::{Foundation::CloseHandle,Security::{GetTokenInformation,TokenIntegrityLevel,TOKEN_QUERY,TOKEN_MANDATORY_LABEL,GetSidSubAuthorityCount,GetSidSubAuthority},System::Threading::{GetCurrentProcess,OpenProcessToken}};
    let mut token=std::ptr::null_mut();ensure!(unsafe{OpenProcessToken(GetCurrentProcess(),TOKEN_QUERY,&mut token)}!=0,"cannot query acceptance token");
    let result=(||->Result<()> {
        let mut length=0;unsafe{GetTokenInformation(token,TokenIntegrityLevel,std::ptr::null_mut(),0,&mut length);}
        ensure!(length>=std::mem::size_of::<TOKEN_MANDATORY_LABEL>() as u32&&length<=4096,"invalid token integrity metadata length");
        let mut buffer=vec![0u64;(length as usize+7)/8];
        ensure!(unsafe{GetTokenInformation(token,TokenIntegrityLevel,buffer.as_mut_ptr().cast(),length,&mut length)}!=0,"cannot read acceptance token integrity");
        let label=unsafe{&*buffer.as_ptr().cast::<TOKEN_MANDATORY_LABEL>()};let count=unsafe{*GetSidSubAuthorityCount(label.Label.Sid)};
        ensure!(count>0,"missing token integrity SID component");let level=unsafe{*GetSidSubAuthority(label.Label.Sid,u32::from(count-1))};
        ensure!(level>=0x2000,"acceptance runner has low integrity; copy this executable to an isolated runtime directory, set only its file label to M, then run from a normal shell instead of cargo run");
        ensure!(!pcscope_ipc::elevated()?,"acceptance requires an ordinary, non-elevated token");Ok(())
    })();unsafe{CloseHandle(token);}result
}
#[tokio::main]async fn main()->Result<()> {
    ordinary_token()?;
    let bundle=std::fs::canonicalize(std::env::args_os().nth(1).ok_or_else(||anyhow::anyhow!("explicit bundle directory required"))?)?;
    let absence=std::env::args().nth(2).as_deref()==Some("--absence-replay");let mut absent_instance=None;
    let root=std::env::current_dir()?.join("target/native-package-smoke").join(format!("{}-{}",std::process::id(),chrono::Utc::now().timestamp_nanos_opt().unwrap()));
    std::fs::create_dir_all(&root)?;let endpoint=Endpoint::for_directory(&root)?;
    ensure!(call_on(&endpoint,Request::Status).await.is_err(),"fresh isolated endpoint unexpectedly exists");
    {
        let mut store=Store::open_program(root.join("history.db"))?;let mut config=json!({});
        for id in ["pcscope-software.exe","pcscope-powershell-adapter.exe","system","security","network","storage","startup","desktop","virtualization","applications","scoped","eventlog","usn"] {
            config[id]=json!({"enabled":id=="pcscope-powershell-adapter.exe"||id=="network","seconds":604800});
        }
        store.save_plugin_settings(&config)?;
        store.set_runtime_watch_settings(false,vec!["sihost".into()],&chrono::Utc::now().to_rfc3339())?;
        if absence {
            let fixture:serde_json::Value=serde_json::from_slice(&std::fs::read("target/native-absence-replay/fixture.json")?)?;
            let machine=fixture["machine"].as_str().ok_or_else(||anyhow::anyhow!("fixture machine missing"))?;let installation=store.projection_installation(machine)?;
            store.ingest(&pcscope_contracts::parse(&fixture["before"].to_string())?)?;
            loop{let report=store.project_pending(machine,&installation,64)?;ensure!(report.rejected==0&&report.unadapted==0,"fixture baseline rejected");if report.examined==0{break}}
            absent_instance=Some(store.reader().query_row("SELECT instance_id FROM source_bindings WHERE source='pcscope.hardware.devices' AND item_id=? LIMIT 1",[fixture["id"].as_str().unwrap()],|row|row.get::<_,i64>(0))?);
        }
    }
    let mut command=Command::new(bundle.join("pcscope-service.exe"));command.arg(&root).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::from(std::fs::File::create(root.join("daemon.stderr.log"))?));
    use std::os::windows::process::CommandExt;command.creation_flags(0x08000000);
    let mut daemon=Daemon(command.spawn()?);let deadline=Instant::now()+Duration::from_secs(180);
    let mut counts=json!({});
    loop {
        ensure!(daemon.0.try_wait()?.is_none(),"isolated packaged daemon exited");
        match call_on(&endpoint,Request::Status).await {
        Ok(status)=>{
            std::fs::write(root.join("latest-status.json"),serde_json::to_vec_pretty(&status)?)?;
            let low_integrity=status["last_collection"]["plugins"].as_array().is_some_and(|rows|rows.iter().any(|row|row["gaps"].as_array().is_some_and(|gaps|gaps.iter().any(|gap|gap.as_str().is_some_and(|text|text.contains("Collector process integrity below ordinary application level"))))));
            ensure!(!low_integrity,"packaged collector integrity below ordinary application level; stage detached runtime files with -SetRuntimeIntegrity");
            let collected=status["last_collection"]["plugins"].as_array().is_some_and(|rows|rows.iter().any(|row|row["plugin"]=="pcscope-powershell-adapter.exe")&&rows.iter().any(|row|row["plugin"]=="network"));
            if collected&&status["collecting"]==false&&status["projection_worker"]["state"]=="idle" {
                let devices=count(&endpoint,"hardware.device").await?;
                let adapters=count(&endpoint,"network.adapter").await?;
                if devices>0&&adapters>0 {counts=json!({"hardware.device":devices,"network.adapter":adapters});break;}
            }
        },Err(error)=>{std::fs::write(root.join("latest-ipc-error.txt"),format!("{error:#}"))?;}
        }
        ensure!(Instant::now()<deadline,"native device/adapter objects did not become queryable");
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    let projection=call_on(&endpoint,Request::Query{request:QueryRequest::ProjectionStatus}).await?;
    ensure!(projection["data"].as_array().is_some_and(|rows|rows.iter().all(|row|row["pending"]==0&&row["failed"]==0)),"live projection backlog or failed work remains");
    let absence_timeline=if let Some(instance)=absent_instance {
        let timeline=call_on(&endpoint,Request::Query{request:QueryRequest::ActionTimeline(pcscope_contracts::query::ActionFilter{instance:Some(instance.to_string()),action_id:Some("observed.no_longer_observed".into()),..Default::default()})}).await?;
        ensure!(timeline["data"].as_array().is_some_and(|rows|rows.len()==1&&rows[0]["precision"]=="interval"&&rows[0]["occurred_at"].is_null()),"real native collection did not create one interval absence action");
        let store=Store::open_reader(root.join("history.db"),Duration::from_secs(2))?;
        let lifecycle:String=store.reader().query_row("SELECT lifecycle FROM object_instances WHERE instance_id=?",[instance],|row|row.get(0))?;
        ensure!(lifecycle=="not_observed","real native absence did not update object lifecycle");timeline
    }else{serde_json::Value::Null};
    call_on(&endpoint,Request::Shutdown).await?;let deadline=Instant::now()+Duration::from_secs(30);
    loop {if let Some(status)=daemon.0.try_wait()?{ensure!(status.success(),"isolated daemon shutdown failed");break}ensure!(Instant::now()<deadline,"isolated daemon shutdown timed out");tokio::time::sleep(Duration::from_millis(100)).await;}
    let summary=json!({"bundle":bundle,"data":root,"counts":counts,"projection":projection,"absence_timeline":absence_timeline,"prior_presence_reconstructed":absence,"formal_endpoint_accessed":false,"existing_database_accessed":false,"system_changes":false,"coverage":"base and network native units only; no pending compatibility units"});
    std::fs::write(root.join("summary.json"),serde_json::to_vec_pretty(&summary)?)?;println!("{summary}");Ok(())
}
