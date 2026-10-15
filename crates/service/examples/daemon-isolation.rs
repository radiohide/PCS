//! Ordinary-user acceptance against two actual daemons, with all collectors disabled in fresh DBs.
use anyhow::{ensure, Result};
use pcscope_ipc::{call_on, Endpoint, Request};
use serde_json::json;
use std::{path::Path, process::{Child, Command, Stdio}, time::Duration};

struct Daemon(Child);
impl Drop for Daemon { fn drop(&mut self) { if self.0.try_wait().ok().flatten().is_none() { let _=self.0.kill();let _=self.0.wait(); } } }
fn launch(exe:&Path,data:&Path,replace:bool)->Result<Daemon> {
    let mut cmd=Command::new(exe);cmd.arg(data);
    if replace {cmd.arg("--replace").arg(pcscope_ipc::user_sid()?);}
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::from(std::fs::File::create(data.join(if replace{"replacement.stderr.log"}else{"daemon.stderr.log"}))?));
    use std::os::windows::process::CommandExt;cmd.creation_flags(0x08000000);
    Ok(Daemon(cmd.spawn()?))
}
async fn ready(endpoint:&Endpoint,child:&mut Daemon)->Result<serde_json::Value> {
    for _ in 0..100 {
        ensure!(child.0.try_wait()?.is_none(),"test daemon exited before ready");
        if let Ok(status)=call_on(endpoint,Request::Status).await {return Ok(status);}
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    anyhow::bail!("isolated daemon did not become ready")
}
async fn stopped(child:&mut Daemon)->Result<()> {
    for _ in 0..300 {
        if let Some(status)=child.0.try_wait()? {ensure!(status.success(),"daemon exited unsuccessfully");return Ok(());}
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    anyhow::bail!("isolated daemon did not stop")
}
#[tokio::main]
async fn main()->Result<()> {
    let exe=std::env::current_exe()?.parent().unwrap().parent().unwrap().join("pcscope-service.exe");
    let root=std::env::current_dir()?.join("target/daemon-isolation").join(format!("{}-{}",std::process::id(),chrono::Utc::now().timestamp_millis()));
    for name in ["实例甲","实例乙"] {
        let dir=root.join(name);std::fs::create_dir_all(&dir)?;
        let store=pcscope_storage::Store::open_program(dir.join("history.db"))?;
        let mut config=json!({});
        for id in ["pcscope-software.exe","pcscope-powershell-adapter.exe","system","security","network","storage","startup","desktop","virtualization","applications","scoped","eventlog","usn"] {config[id]=json!({"enabled":false,"seconds":300});}
        store.save_plugin_settings(&config)?;
    }
    let a=Endpoint::for_directory(&root.join("实例甲"))?;let b=Endpoint::for_directory(&root.join("实例乙"))?;
    let mut first=launch(&exe,&a.data_dir,false)?;let mut second=launch(&exe,&b.data_dir,false)?;
    let one=ready(&a,&mut first).await?;let two=ready(&b,&mut second).await?;
    ensure!(one["pid"]!=two["pid"]&&one["instance_id"]!=two["instance_id"],"instances collided");
    call_on(&a,Request::SetFileWatchSettings{files:vec![r"C:\isolated-fixture\one.dll".into()]}).await?;
    ensure!(call_on(&b,Request::FileWatchSettings).await?["files"]==json!([]),"settings leaked across directories");
    let mut duplicate=launch(&exe,&a.data_dir,false)?;
    for _ in 0..100 {if duplicate.0.try_wait()?.is_some(){break;}tokio::time::sleep(Duration::from_millis(50)).await;}
    ensure!(duplicate.0.try_wait()?.is_some_and(|s|!s.success()),"duplicate daemon accepted");
    let mut replacement=launch(&exe,&a.data_dir,true)?;
    stopped(&mut first).await?;
    let replaced=ready(&a,&mut replacement).await?;
    ensure!(replaced["pid"]!=one["pid"]&&replaced["instance_id"]==one["instance_id"],"replacement targeted wrong instance");
    ensure!(call_on(&b,Request::Status).await?["pid"]==two["pid"],"replacement stopped the other daemon");
    ensure!(call_on(&a,Request::FileWatchSettings).await?["files"].as_array().unwrap().len()==1,"restart lost settings");
    call_on(&a,Request::Shutdown).await?;stopped(&mut replacement).await?;
    ensure!(call_on(&b,Request::Status).await?["pid"]==two["pid"],"shutdown stopped the other daemon");
    call_on(&b,Request::Shutdown).await?;stopped(&mut second).await?;
    std::fs::write(root.join("result.json"),serde_json::to_vec_pretty(&json!({"passed":true,"first":one,"second":two,"replacement":replaced,"checks":["distinct endpoints","settings isolation","duplicate rejected","same-account replacement isolation","restart persistence","shutdown isolation"]}))?)?;
    println!("PASS: actual daemon isolation, duplicate rejection, replacement, restart and shutdown; {}",root.display());
    Ok(())
}
