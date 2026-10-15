use std::{path::{Path,PathBuf},sync::{Mutex,atomic::{AtomicU64,Ordering}}};
use serde_json::{json,Value};

pub struct Settings {
    config:PathBuf,
    data:PathBuf,
    runtime:PathBuf,
    webview:Option<PathBuf>,
    overrides:[bool;2],
    save_lock:Mutex<()>,
}

const REQUIRED:&[&str]=&["pcscope-service.exe","pcscope-software.exe","pcscope-powershell-adapter.exe","pcscope-eventlog.exe","pcscope-eventlog-native.exe","pcscope-platform-native.exe","pcscope-usn.exe","collect-system.ps1","collect-domain.ps1","evidence.ps1","collect-eventlog.ps1","event-schema.ps1","channels.json","collector-options.json"];

impl Settings {
    pub fn new(data:PathBuf)->Result<Self,String>{
        let exe=std::env::current_exe().map_err(|e|e.to_string())?;
        Ok(Self{config:exe.with_file_name("pcscope-launch.json"),data,runtime:crate::launch::runtime_dir()?,
            webview:std::env::var_os("WEBVIEW2_USER_DATA_FOLDER").map(PathBuf::from),
            overrides:*crate::launch::OVERRIDES.get().unwrap_or(&[false,false]),save_lock:Mutex::new(())})
    }
    fn read(&self)->Result<Value,String>{
        let saved=crate::launch::read_defaults(&self.config)?;
        let next_data=if self.overrides[0]{&self.data}else{saved.data_dir.as_ref().unwrap_or(&self.data)};
        let next_runtime=if self.overrides[1]{&self.runtime}else{saved.runtime_dir.as_ref().unwrap_or(&self.runtime)};
        Ok(json!({"current":{"data_dir":self.data,"runtime_dir":self.runtime,"webview_dir":self.webview},
            "saved":{"data_dir":next_data,"runtime_dir":next_runtime},"config_file":self.config,
            "overrides":{"data_dir":self.overrides[0],"runtime_dir":self.overrides[1]},
            "restart_required":next_data!=&self.data||next_runtime!=&self.runtime}))
    }
    fn save(&self,data:&str,runtime:&str)->Result<Value,String>{
        let _lock=self.save_lock.lock().map_err(|_|"启动配置正在处理，请稍后重试。")?;
        let data=PathBuf::from(data.trim());let runtime=PathBuf::from(runtime.trim());
        if !data.is_absolute()||!runtime.is_absolute(){return Err("请输入完整的目录路径。".into());}
        if !data.is_dir(){return Err("数据目录不存在，请先创建或选择已有目录。".into());}
        if self.overrides[0]&&data!=self.data||self.overrides[1]&&runtime!=self.runtime {
            return Err("该目录由启动参数或环境变量指定，请先调整启动方式。".into());
        }
        validate_runtime(&runtime)?;
        // Saving never changes the active endpoint, moves history, starts a daemon or creates a DB.
        let previous=crate::launch::read_defaults(&self.config)?;
        let mut value=json!({"version":1,"data_dir":data,"runtime_dir":runtime});
        if let Some(cache)=previous.webview_dir.or_else(||self.webview.clone()){value["webview_dir"]=json!(cache);}
        atomic_save(&self.config,&serde_json::to_vec_pretty(&value).map_err(|e|e.to_string())?)?;
        self.read()
    }
}

fn validate_runtime(path:&Path)->Result<(),String>{
    if !path.is_dir(){return Err("后台及采集程序目录不存在。".into());}
    if REQUIRED.iter().any(|name|!path.join(name).is_file())||!path.join("domains").is_dir(){
        return Err("该目录的后台或采集文件不完整，请选择完整程序目录。".into());
    }
    Ok(())
}

fn atomic_save(path:&Path,bytes:&[u8])->Result<(),String>{
    use std::io::Write;
    static SEQUENCE:AtomicU64=AtomicU64::new(0);
    let temporary=path.with_file_name(format!("pcscope-launch.{}-{}.pending",std::process::id(),SEQUENCE.fetch_add(1,Ordering::Relaxed)));
    let mut file=std::fs::OpenOptions::new().create_new(true).write(true).open(&temporary)
        .map_err(|e|format!("无法保存启动配置：{e}"))?;
    let result=(||{
        file.write_all(bytes)?;file.sync_all()?;drop(file);
        #[cfg(windows)] {
            use std::os::windows::ffi::OsStrExt;
            use windows_sys::Win32::Storage::FileSystem::{MoveFileExW,MOVEFILE_REPLACE_EXISTING,MOVEFILE_WRITE_THROUGH};
            let from:Vec<u16>=temporary.as_os_str().encode_wide().chain(Some(0)).collect();
            let to:Vec<u16>=path.as_os_str().encode_wide().chain(Some(0)).collect();
            if unsafe{MoveFileExW(from.as_ptr(),to.as_ptr(),MOVEFILE_REPLACE_EXISTING|MOVEFILE_WRITE_THROUGH)}==0{return Err(std::io::Error::last_os_error());}
        }
        #[cfg(not(windows))] std::fs::rename(&temporary,path)?;
        Ok::<_,std::io::Error>(())
    })();
    if result.is_err(){let _=std::fs::remove_file(&temporary);}
    result.map_err(|e|format!("无法保存启动配置，原配置保留：{e}"))
}

#[tauri::command]
pub fn get_launch_settings(settings:tauri::State<'_,Settings>)->Result<Value,String>{settings.read()}

#[tauri::command]
pub fn set_launch_settings(settings:tauri::State<'_,Settings>,data_dir:String,runtime_dir:String)->Result<Value,String>{settings.save(&data_dir,&runtime_dir)}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn saves_next_launch_only_and_rejects_incomplete_runtime_without_overwriting(){
        let root=std::env::temp_dir().join(format!("pcscope-launch-settings-{}",std::process::id()));
        std::fs::create_dir_all(root.join("runtime/domains")).unwrap();std::fs::create_dir_all(root.join("new-data")).unwrap();
        for name in REQUIRED {std::fs::write(root.join("runtime").join(name),b"fixture").unwrap();}
        let mut settings=Settings{config:root.join("pcscope-launch.json"),data:root.clone(),runtime:root.join("runtime"),webview:Some(root.join("cache")),overrides:[false,false],save_lock:Mutex::new(())};
        let report=settings.save(root.join("new-data").to_str().unwrap(),settings.runtime.to_str().unwrap()).unwrap();
        assert_eq!(report["restart_required"],true);assert_eq!(report["current"]["data_dir"],json!(root));
        assert!(!root.join("new-data/history.db").exists());
        let before=std::fs::read(&settings.config).unwrap();
        assert!(settings.save(root.to_str().unwrap(),root.to_str().unwrap()).is_err());
        assert_eq!(std::fs::read(&settings.config).unwrap(),before);
        settings.overrides[0]=true;
        assert!(settings.save(root.join("new-data").to_str().unwrap(),settings.runtime.to_str().unwrap()).is_err());
        assert_eq!(std::fs::read(&settings.config).unwrap(),before);
        // A second valid save exercises atomic replacement of an existing configuration.
        settings.overrides[0]=false;settings.save(root.to_str().unwrap(),settings.runtime.to_str().unwrap()).unwrap();
        assert_eq!(settings.read().unwrap()["restart_required"],false);
        assert_eq!(root.canonicalize().unwrap().parent().unwrap(),std::env::temp_dir().canonicalize().unwrap());
        std::fs::remove_dir_all(root).unwrap();
    }
}
