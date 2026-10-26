use super::*;
use std::io::Write;
static REQUEST_LOCK:Mutex<()>=Mutex::new(());
const RECEIPT:&str="history-deletion.json";

pub fn status(directory:&std::path::Path)->Result<Value>{
    match std::fs::read(directory.join(RECEIPT)) {
        Ok(bytes)=>{anyhow::ensure!(bytes.len()<16384,"删除进度文件异常");Ok(serde_json::from_slice(&bytes)?)},
        Err(e)if e.kind()==std::io::ErrorKind::NotFound=>Ok(json!({"state":"idle"})),Err(e)=>Err(e.into())
    }
}
fn save(directory:&std::path::Path,value:&Value)->Result<()> {
    let path=directory.join(format!(".history-deletion-{}.tmp",chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()));
    let mut file=std::fs::OpenOptions::new().write(true).create_new(true).open(&path)?;
    file.write_all(&serde_json::to_vec(value)?)?;file.sync_all()?;drop(file);
    std::fs::rename(path,directory.join(RECEIPT))?;Ok(())
}
pub fn request(state:&Arc<State>,history_id:&str,confirmation:&str)->Result<Value>{
    anyhow::ensure!(confirmation=="DELETE ALL HISTORY","请先确认删除全部历史数据");
    let _lock=REQUEST_LOCK.try_lock().map_err(|_|anyhow::anyhow!("删除操作正在准备，请稍候"))?;
    let previous=status(&state.data_dir)?;
    if previous["state"]=="pending" {
        anyhow::ensure!(previous["history_id"]==history_id,"已有删除操作正在进行");return Ok(previous);
    }
    anyhow::ensure!(!state.stopping.load(Ordering::SeqCst),"后台正在退出，请稍后重试");
    anyhow::ensure!(state.store.lock().map_err(|_|anyhow::anyhow!("storage lock poisoned"))?.history_id()?==history_id,"历史数据已变化，请重新打开历史存储后确认");
    let id=format!("{:032x}{:08x}",chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default(),std::process::id());
    let mut receipt=json!({"state":"pending","id":id,"history_id":history_id,"instance_id":state.endpoint.instance_id,"at":chrono::Utc::now().to_rfc3339()});
    save(&state.data_dir,&receipt)?;
    let spawned=(||->Result<()>{
        let mut command=std::process::Command::new(std::env::current_exe()?);
        command.arg(&state.data_dir).arg("--replace").arg(pcscope_ipc::user_sid()?);
        #[cfg(windows)] {use std::os::windows::process::CommandExt;command.creation_flags(0x08000000);}
        command.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn()?;Ok(())
    })();
    if let Err(error)=spawned {receipt["state"]=json!("failed");receipt["error"]=json!("无法启动清理程序，历史数据保留，请重试。");save(&state.data_dir,&receipt)?;return Err(error);}
    Ok(receipt)
}

/// Called only after exclusive ownership of the selected daemon endpoint, before any worker.
pub fn resume(directory:&std::path::Path,instance:&str)->Result<()> {
    let mut receipt=status(directory)?;if receipt["state"]!="pending"{return Ok(())}
    anyhow::ensure!(receipt["instance_id"]==instance,"删除操作不属于当前数据目录");
    let result=pcscope_storage::history_reset::replace_history(directory,receipt["id"].as_str().context("missing operation id")?,receipt["history_id"].as_str().context("missing history identity")?);
    match result {
        Ok(())=>{receipt["state"]=json!("completed");receipt["completed_at"]=json!(chrono::Utc::now().to_rfc3339());},
        Err(error)=>{receipt["state"]=json!("failed");receipt["error"]=json!(format!("删除未完成，未替换历史数据库。请关闭其他数据库工具后重试。详情：{error}"));}
    }
    save(directory,&receipt)
}
