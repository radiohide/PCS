#![windows_subsystem = "windows"]
use tauri::Manager;
mod service_control;
mod launch;
mod launch_settings;
use service_control::ServiceControl;
use std::sync::Arc;

fn connection_error(kind: Option<std::io::ErrorKind>, detail: String) -> String {
    if kind==Some(std::io::ErrorKind::NotFound) {
        "后台尚未连接，监控信息暂时无法更新。".into()
    } else {detail}
}

// A slow or rejected reply does not mean that the selected pipe is absent. In that case a
// second daemon cannot help, and repeatedly waiting for it also blocks the startup screen.
fn startup_probe_error(kind: Option<std::io::ErrorKind>, detail: String) -> Option<String> {
    match kind {
        Some(std::io::ErrorKind::NotFound) => None,
        Some(std::io::ErrorKind::PermissionDenied) => Some("后台连接被拒绝，监控信息无法更新。请从正常应用目录重新打开程序。开发预览请使用 open-preview.ps1。".into()),
        _ => Some(connection_error(kind, detail)),
    }
}

async fn launch_selected_daemon(endpoint:&pcscope_ipc::Endpoint,target:bool)->Result<serde_json::Value,String> {
    let executable=launch::service_executable()?;
    if !executable.is_file(){return Err("后台程序不存在".into());}
    let data=endpoint.data_dir.clone();let sid=pcscope_ipc::user_sid().map_err(|e|e.to_string())?;
    let pid=tauri::async_runtime::spawn_blocking(move ||pcscope_ipc::launch_daemon(&executable,&data,&sid,target))
        .await.map_err(|e|e.to_string())?.map_err(|e|format!("授权或后台启动未完成：{e}"))?;
    Ok(serde_json::json!({"switching":true,"pid":pid,"elevated":target}))
}

#[tauri::command]
async fn switch_service(endpoint: tauri::State<'_,pcscope_ipc::Endpoint>, control: tauri::State<'_,Arc<ServiceControl>>, mode: String) -> Result<serde_json::Value,String> {
    let mut launch=control.0.try_lock().map_err(|_|"后台正在启动或切换，请稍候。".to_string())?;
    if mode!="enhanced" && mode!="low" {return Err("未知运行模式".into());}
    let current=pcscope_ipc::call_on(&endpoint,pcscope_ipc::Request::MonitoringSettings).await.map_err(|e|format!("{e:#}"))?;
    if current["settings"]["mode"]!=mode {return Err("请先保存运行模式".into());}
    let target=mode=="enhanced";
    if current["elevated"]==target {
        return pcscope_ipc::call_on(&endpoint,pcscope_ipc::Request::CheckMonitoring).await.map_err(|e|e.to_string());
    }
    if !target && pcscope_ipc::elevated().map_err(|e|e.to_string())? {return Err("桌面本身以管理员运行，无法降权。请关闭桌面并以普通方式重新打开，再切换低权限运行。".into());}
    if current["instance_id"]!=endpoint.instance_id {return Err("后台实例身份不匹配".into());}
    let result=launch_selected_daemon(&endpoint,target).await;
    // Replacement may wait up to 180 seconds for the old daemon to drain. Never race it
    // with an ordinary auto-start, including after the UI's shorter waiting period ends.
    launch.defer(std::time::Instant::now(),if result.is_ok(){180}else{10});
    result
}

#[tauri::command]
async fn request(endpoint: tauri::State<'_,pcscope_ipc::Endpoint>, request:pcscope_ipc::Request)->Result<serde_json::Value,String> {
    pcscope_ipc::call_on(&endpoint,request).await.map_err(|error|connection_error(
        error.downcast_ref::<std::io::Error>().map(std::io::Error::kind),format!("{error:#}")))
}

#[tauri::command]
async fn start_service(endpoint: tauri::State<'_,pcscope_ipc::Endpoint>, control: tauri::State<'_,Arc<ServiceControl>>, manual: Option<bool>)->Result<serde_json::Value,String> {
    ensure_service(&endpoint,&control,manual.unwrap_or(false)).await
}

async fn ensure_service(endpoint: &pcscope_ipc::Endpoint, control: &ServiceControl, manual: bool)->Result<serde_json::Value,String> {
    let mut launch=control.0.try_lock().map_err(|_|"后台正在启动或切换，请稍候。".to_string())?;
    match pcscope_ipc::call_on(&endpoint,pcscope_ipc::Request::Status).await {
        Ok(status)=>{launch.connected(std::time::Instant::now());return Ok(status);},
        Err(error)=>{
            launch.disconnected();
            if let Some(message)=startup_probe_error(error.downcast_ref::<std::io::Error>().map(std::io::Error::kind),format!("{error:#}")) {
                return Err(message);
            }
        },
    }
    if !launch.allowed(std::time::Instant::now()) {return Err("后台正在恢复或切换权限，请稍候。".into());}
    if !launch.child_running()? {
        launch.reserve_attempt(std::time::Instant::now(),manual)?;
        let executable=launch::service_executable()?;
        if !executable.is_file() {return Err("后台程序缺失，请重新安装或补齐程序文件。".into());}
        let mut command=std::process::Command::new(executable);
        command.arg(&endpoint.data_dir).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        #[cfg(windows)] {use std::os::windows::process::CommandExt;command.creation_flags(0x08000000);}
        launch.child=Some(command.spawn().map_err(|e|format!("后台未能启动：{e}"))?);
    }
    // One overall deadline, not 50 individually timed-out IPC calls. Keep the child handle
    // on timeout so a slow startup cannot lead to another process on the next attempt.
    tokio::time::timeout(std::time::Duration::from_secs(20),async {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            if let Ok(status)=pcscope_ipc::call_on(&endpoint,pcscope_ipc::Request::Status).await {return Ok(status);}
            if !launch.child_running()? {return Err("后台启动后已退出，请重试或检查程序文件与数据目录权限。".into());}
        }
    }).await.unwrap_or_else(|_|Err("后台尚未就绪，稍后将再次尝试连接。已有记录仍保留。".into()))
}

fn main() {
    if let Err(error)=launch::configure() {
        launch::report_failure(&error);std::process::exit(1);
    }
    // Tauri can panic on a setup-hook error before returning its Result. Surface both paths
    // to a double-click user, who otherwise sees the process disappear without an explanation.
    let result=std::panic::catch_unwind(run);
    let error=match result {
        Ok(Ok(()))=>return,
        Ok(Err(error))=>error.to_string(),
        Err(error)=>error.downcast_ref::<String>().cloned()
            .or_else(||error.downcast_ref::<&str>().map(|text|(*text).to_owned()))
            .unwrap_or_else(||"应用初始化失败".into()),
    };
    launch::report_failure(&error);std::process::exit(1);
}

fn run() -> tauri::Result<()> {
    tauri::Builder::default().setup(|app| {
        let data=match std::env::var_os("PCSCOPE_DATA_DIR") {Some(path)=>std::path::PathBuf::from(path),None=>app.path().app_local_data_dir()?.join("native-v1")};
        std::fs::create_dir_all(&data)?;
        let endpoint=pcscope_ipc::Endpoint::for_directory(&data)?;
        app.manage(launch_settings::Settings::new(endpoint.data_dir.clone()).map_err(std::io::Error::other)?);
        app.manage(endpoint.clone());
        let control=Arc::new(ServiceControl::default());
        app.manage(control.clone());
        tauri::async_runtime::spawn(async move {
            loop {
                // Native recovery also runs while WebView timers are suspended/minimized.
                // Recovery uses no shell or privilege prompt and never opens the database itself.
                if pcscope_ipc::call_on(&endpoint,pcscope_ipc::Request::RegisterFrontend {pid:std::process::id()}).await.is_ok() {
                    if let Ok(mut launch)=control.0.try_lock(){launch.connected(std::time::Instant::now());}
                } else {
                    let _=ensure_service(&endpoint,&control,false).await;
                }
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            }
        });
        Ok(())
    }).invoke_handler(tauri::generate_handler![request,start_service,switch_service,launch_settings::get_launch_settings,launch_settings::set_launch_settings]).run(tauri::generate_context!())
}

#[cfg(test)]
mod tests {
    use super::{connection_error,startup_probe_error};
    #[test]
    fn missing_pipe_is_not_reported_as_missing_application_file() {
        assert!(connection_error(Some(std::io::ErrorKind::NotFound),"os error 2".into()).contains("后台尚未连接"));
        assert_eq!(connection_error(None,"collector file missing".into()),"collector file missing");
        assert_eq!(connection_error(Some(std::io::ErrorKind::PermissionDenied),"access denied".into()),"access denied");
    }

    #[test]
    fn only_a_missing_pipe_allows_starting_another_daemon() {
        use std::io::ErrorKind;
        assert!(startup_probe_error(Some(ErrorKind::NotFound),"pipe absent".into()).is_none());
        for kind in [None,Some(ErrorKind::TimedOut),Some(ErrorKind::PermissionDenied),Some(ErrorKind::BrokenPipe),Some(ErrorKind::InvalidData),Some(ErrorKind::Other)] {
            assert!(startup_probe_error(kind,"existing endpoint did not answer".into()).is_some(),"must not start a duplicate backend for {kind:?}");
        }
        assert_eq!(startup_probe_error(None,"deadline has elapsed".into()),Some("deadline has elapsed".into()));
    }
}
