//! End-to-end native file tracking fixture. Never modifies SIHost or the real Shell.
use anyhow::{ensure,Context,Result};
use pcscope_contracts::query::{QueryRequest,InstanceFilter};
use pcscope_ipc::{Endpoint,Request,call_on};
use serde_json::{json,Value};
use std::{path::{Path,PathBuf},process::{Child,Command,Stdio},time::{Duration,Instant}};
struct OwnedChild(Child);
impl Drop for OwnedChild{fn drop(&mut self){if self.0.try_wait().ok().flatten().is_none(){let _=self.0.kill();let _=self.0.wait();}}}
fn spawn(exe:&Path,args:&[&Path],stderr:&Path,stdin:bool)->Result<OwnedChild>{
    let mut c=Command::new(exe);c.args(args).stdin(if stdin{Stdio::piped()}else{Stdio::null()}).stdout(Stdio::null()).stderr(Stdio::from(std::fs::File::create(stderr)?));
    use std::os::windows::process::CommandExt;c.creation_flags(0x08000000);Ok(OwnedChild(c.spawn()?))
}
async fn readings(endpoint:&Endpoint)->Result<Value>{call_on(endpoint,Request::Query{request:QueryRequest::SourceReadings{source:Some("pcscope.execution.module-file-state".into()),cursor:None,limit:200}}).await}
async fn ready(endpoint:&Endpoint,daemon:&mut OwnedChild)->Result<()> {
    for _ in 0..100 {if call_on(endpoint,Request::Status).await.is_ok(){return Ok(());}ensure!(daemon.0.try_wait()?.is_none(),"daemon failed to start");tokio::time::sleep(Duration::from_millis(100)).await;}
    anyhow::bail!("isolated daemon did not become ready")
}
async fn wait_check(endpoint:&Endpoint,path:&str,predicate:impl Fn(&Value)->bool)->Result<Value>{
    let end=Instant::now()+Duration::from_secs(60);
    while Instant::now()<end {
        let result=readings(endpoint).await?;
        if let Some(row)=result["data"].as_array().and_then(|rows|rows.iter().find(|r|r["payload"]["path"].as_str().is_some_and(|p|p.replace('/',"\\").eq_ignore_ascii_case(&path.replace('/',"\\")))&&predicate(&r["payload"]))) {return Ok(row.clone());}
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    anyhow::bail!("expected file observation did not arrive: {path}")
}
async fn shutdown(endpoint:&Endpoint,daemon:&mut OwnedChild)->Result<()> {
    call_on(endpoint,Request::Shutdown).await?;
    let end=Instant::now()+Duration::from_secs(30);
    while Instant::now()<end {if let Some(status)=daemon.0.try_wait()?{ensure!(status.success(),"daemon shutdown failed");return Ok(());}tokio::time::sleep(Duration::from_millis(100)).await;}
    anyhow::bail!("daemon shutdown timed out")
}
fn cpu_seconds(child:&Child)->Result<f64> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::{Foundation::FILETIME,System::Threading::GetProcessTimes};
    let mut created=FILETIME{dwLowDateTime:0,dwHighDateTime:0};let mut exited=created;let mut kernel=created;let mut user=created;
    ensure!(unsafe{GetProcessTimes(child.as_raw_handle(),&mut created,&mut exited,&mut kernel,&mut user)}!=0,"test process CPU read failed");
    let ticks=|t:FILETIME|((t.dwHighDateTime as u64)<<32)|t.dwLowDateTime as u64;
    Ok((ticks(kernel)+ticks(user)) as f64/10_000_000.)
}
#[tokio::main]async fn main()->Result<()> {
    let fixture=PathBuf::from(std::env::args_os().nth(1).context("fixture directory required")?);
    let exe=std::env::current_exe()?.parent().unwrap().parent().unwrap().join("pcscope-service.exe");
    let root=std::env::current_dir()?.join("target/sihost-file-monitor").join(format!("{}-{}",std::process::id(),chrono::Utc::now().timestamp_millis()));
    let data=root.join("isolated-data");let files=root.join("module-files");std::fs::create_dir_all(&data)?;std::fs::create_dir_all(&files)?;
    let module=files.join("example-third-party.dll");std::fs::copy(fixture.join("module1.dll"),&module)?;
    let time=chrono::Utc::now().to_rfc3339();
    {let mut store=pcscope_storage::Store::open_program(data.join("history.db"))?;let mut jobs=json!({});
        for id in ["pcscope-software.exe","pcscope-powershell-adapter.exe","system","security","network","storage","startup","desktop","virtualization","applications","scoped","eventlog","usn"]{jobs[id]=json!({"enabled":id=="scoped","seconds":300});}store.save_plugin_settings(&jobs)?;
        let controls=store.item_controls()?;
        for row in controls["items"].as_array().unwrap(){let id=row["id"].as_str().unwrap();if !["pcscope.execution.modules","pcscope.execution.module-scan-status","pcscope.execution.module-file-state"].contains(&id){store.set_item_enabled(row["plugin"].as_str().unwrap(),id,false,&time)?;}}
        store.set_runtime_watch_settings(true,vec!["pcscope-module-fixture".into()],&time)?;
        store.set_file_watch_settings(vec![files.join("manual-only.txt").to_string_lossy().into_owned()],&time)?;
    }
    let host=fixture.join("pcscope-module-fixture.exe");let mut first=spawn(&host,&[&module],&root.join("host1.stderr.log"),true)?;let mut second=spawn(&host,&[&module],&root.join("host2.stderr.log"),true)?;
    tokio::time::sleep(Duration::from_millis(300)).await;ensure!(first.0.try_wait()?.is_none()&&second.0.try_wait()?.is_none(),"fixture DLL failed to load");
    let endpoint=Endpoint::for_directory(&data)?;let mut daemon=spawn(&exe,&[&data],&root.join("daemon.stderr.log"),false)?;
    ready(&endpoint,&mut daemon).await?;
    let path=module.to_string_lossy().replace('/',"\\");
    let baseline=wait_check(&endpoint,&path,|p|p["check"]["status"]=="stable"&&p["check"]["sha256"].is_string()).await?;
    let objects=call_on(&endpoint,Request::Query{request:QueryRequest::InstanceList(InstanceFilter{type_id:Some("runtime.module_instance".into()),limit:200,..Default::default()})}).await?;
    ensure!(objects["counts"]["instances"].as_u64().unwrap_or(0)>0,"module material did not project");
    let manual=call_on(&endpoint,Request::FileWatchSettings).await?;
    ensure!(manual["files"].as_array().unwrap().len()==1,"automatic watch overwrote manual list");
    let old=files.join("original-loaded.dll");std::fs::rename(&module,&old)?;std::fs::copy(fixture.join("module2.dll"),&module)?;
    let changed=wait_check(&endpoint,&path,|p|p["check"]["status"]=="stable"&&p["check"]["sha256"]!=baseline["payload"]["check"]["sha256"]).await?;
    ensure!(changed["payload"]["check"]["file_id"]!=baseline["payload"]["check"]["file_id"],"replacement did not preserve distinct identities");
    ensure!(first.0.try_wait()?.is_none()&&second.0.try_wait()?.is_none(),"file replacement was mistaken for host exit");
    let moved_path=old.to_string_lossy().replace('/',"\\");
    let moved=wait_check(&endpoint,&moved_path,|p|p["changes"].as_array().is_some_and(|c|c.contains(&json!("file_moved")))).await?;
    ensure!(moved["payload"]["check"]["file_id"]==baseline["payload"]["check"]["file_id"],"renamed file was bound without matching original identity");
    std::fs::write(&module,b"controlled corruption fixture; old mapped DLL remains in the host")?;
    let corrupted=wait_check(&endpoint,&path,|p|p["check"]["pe_status"]=="basic_headers_invalid").await?;
    ensure!(first.0.try_wait()?.is_none()&&second.0.try_wait()?.is_none(),"disk corruption was reported as a host exit");
    let file_objects=call_on(&endpoint,Request::Query{request:QueryRequest::InstanceList(InstanceFilter{type_id:Some("file.watched_file".into()),limit:200,..Default::default()})}).await?;
    let aliases:Vec<_>=file_objects["data"].as_array().unwrap().iter().filter_map(|r|r["alias"].as_str()).collect();
    let old_id=baseline["payload"]["check"]["file_id"].as_str().unwrap();let new_id=changed["payload"]["check"]["file_id"].as_str().unwrap();
    let old_alias=aliases.iter().find(|a|a.contains(old_id)).context("original file was not projected with filesystem identity")?;
    let new_alias=aliases.iter().find(|a|a.contains(new_id)).context("replacement file was not projected with a separate filesystem identity")?;
    ensure!(old_alias!=new_alias,"same-path replacement reused the old model instance");
    std::fs::remove_file(&module)?;let missing=wait_check(&endpoint,&path,|p|p["check"]["status"]=="missing").await?;
    let before=readings(&endpoint).await?["counts"]["source_records"].as_u64().unwrap();
    for _ in 0..50{std::fs::write(files.join("unrelated.txt"),b"no global recollection")?;}
    tokio::time::sleep(Duration::from_secs(2)).await;
    let after=readings(&endpoint).await?["counts"]["source_records"].as_u64().unwrap();ensure!(after-before<10,"unrelated writes caused a collection storm");
    tokio::time::sleep(Duration::from_secs(10)).await;
    let cpu_before=cpu_seconds(&daemon.0)?;let idle_start=Instant::now();tokio::time::sleep(Duration::from_secs(15)).await;
    let idle_seconds=idle_start.elapsed().as_secs_f64();let idle_cpu_seconds=cpu_seconds(&daemon.0)?-cpu_before;
    let processors=std::thread::available_parallelism()?.get();
    // Stop only the fixture hosts. Known files must remain watched with no target process.
    first.0.stdin.take();second.0.stdin.take();first.0.wait()?;second.0.wait()?;
    shutdown(&endpoint,&mut daemon).await?;
    std::fs::copy(fixture.join("module1.dll"),&module)?;
    let mut resumed=spawn(&exe,&[&data],&root.join("resumed.stderr.log"),false)?;
    ready(&endpoint,&mut resumed).await?;
    let restored=wait_check(&endpoint,&path,|p|p["check"]["status"]=="stable"&&p["check"]["file_id"]!=changed["payload"]["check"]["file_id"]).await?;
    ensure!(call_on(&endpoint,Request::FileWatchSettings).await?==manual,"restart altered manual settings");
    shutdown(&endpoint,&mut resumed).await?;
    let saved=pcscope_storage::Store::open(data.join("history.db"))?.runtime_watch_state()?;
    let saved_file=saved["files"][path.to_lowercase()].clone();
    ensure!(saved_file["bindings"].as_object().is_some_and(|b|b.len()==2),"shared DLL lost either process association");
    ensure!(saved_file["first_baseline"]["sha256"]==baseline["payload"]["check"]["sha256"],"replacement overwrote immutable baseline");
    std::fs::write(root.join("result.json"),serde_json::to_vec_pretty(&json!({"passed":true,"module_instances":objects["counts"]["instances"],"file_instances":file_objects["counts"]["instances"],"original_file_alias":old_alias,"replacement_file_alias":new_alias,"baseline":baseline,"replacement":changed,"moved_original":moved,"corrupted_with_hosts_running":corrupted,"missing":missing,"restored_without_target":restored,"manual_settings":manual,"unrelated_write_records_before":before,"unrelated_write_records_after":after,"idle_seconds":idle_seconds,"idle_cpu_seconds":idle_cpu_seconds,"processors":processors,"idle_cpu_machine_percent":idle_cpu_seconds/idle_seconds/processors as f64*100.,"checks":["positive module projection","canonical file projection with distinct replacement identity","shared file associations","native replacement, rename and deletion","disk corruption while host still runs","old baseline retained","manual settings preserved","unrelated notifications bounded","restart with no target keeps known files"]}))?)?;
    println!("PASS: native module/file monitoring end-to-end; {}",root.display());Ok(())
}
