use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

/// An explicit resolved data directory. Never falls back to a different running daemon.
#[derive(Debug, Clone)]
pub struct Endpoint {
    pub data_dir: std::path::PathBuf,
    pub instance_id: String,
    name: String,
}

#[cfg(windows)]
impl Endpoint {
    pub fn for_directory(data: &std::path::Path) -> Result<Self> {
        use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
        use windows_sys::Win32::Storage::FileSystem::{GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, FILE_FLAG_BACKUP_SEMANTICS};
        let resolved = std::fs::canonicalize(data).context("resolving selected data directory")?;
        let text=resolved.to_str().ok_or_else(||anyhow::anyhow!("data directory must have a Unicode path"))?;
        // Win32 collectors compare ordinary paths; avoid an extended-prefix self-watch mismatch.
        let data_dir=std::path::PathBuf::from(if let Some(unc)=text.strip_prefix(r"\\?\UNC\"){format!(r"\\{unc}")}else{text.strip_prefix(r"\\?\").unwrap_or(text).to_owned()});
        anyhow::ensure!(data_dir.is_dir(), "data directory is not a directory");
        // Directory identity also makes junctions/case aliases resolve to the same endpoint.
        let file = std::fs::OpenOptions::new().read(true).custom_flags(FILE_FLAG_BACKUP_SEMANTICS).open(&data_dir).context("reading selected directory identity")?;
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
            return Err(std::io::Error::last_os_error()).context("reading directory file identity");
        }
        let sid = user_sid().context("reading current account identity")?;
        let identity = format!("{sid}/{:08x}/{:08x}{:08x}", info.dwVolumeSerialNumber, info.nFileIndexHigh, info.nFileIndexLow);
        let instance_id = format!("{:x}", Sha256::digest(identity.as_bytes()));
        let name = format!(r"\\.\pipe\PCScope.native.v1.{sid}.{instance_id}");
        Ok(Self { data_dir, instance_id, name })
    }

    pub fn pipe_name(&self) -> &str { &self.name }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    ItemControls,
    SetItemEnabled { plugin:String, id:String, enabled:bool },
    RegisterFrontend { pid: u32 },
    MonitoringSettings,
    FileWatchSettings,
    SetFileWatchSettings { files: Vec<String> },
    DirectoryWatchSettings,
    SetDirectoryWatchSettings { directories: Vec<String> },
    RuntimeWatchSettings,
    SetRuntimeWatchSettings { enabled: bool, module_process_names: Vec<String> },
    CheckMonitoring,
    SetMonitoringSettings { mode: String, repair: bool },
    Status,
    Collect,
    RepairSnapshotAppearance { limit: usize },
    Plugins,
    GlobalCollectionSchedule,
    SetGlobalCollectionSchedule { seconds: u64 },
    SetPlugin {
        id: String,
        enabled: Option<bool>,
        seconds: Option<u64>,
    },
    CollectPlugin {
        id: String,
    },
    Shutdown,
    Retention,
    HistoryDeletionStatus,
    DeleteHistory { history_id: String, confirmation: String },
    PrivacySettings,
    SetPrivacySettings {
        show_sensitive: bool,
    },
    SetRetention {
        mode: String,
        quota_bytes: u64,
    },
    /// The object model's queries, as one entry point.
    ///
    /// The request and the response are R05's own types rather than a copy of them, so the wire
    /// cannot drift from the contract the query layer implements.
    Query {
        request: pcscope_contracts::query::QueryRequest,
    },
}

pub const MAX_RESPONSE: usize = 16 * 1024 * 1024;

fn windows_argument(value:&str)->String {
    let mut out=String::from("\"");let mut slashes=0;
    for c in value.chars(){if c=='\\'{slashes+=1;continue;}out.push_str(&"\\".repeat(if c=='\"'{slashes*2+1}else{slashes}));slashes=0;out.push(c);}
    out.push_str(&"\\".repeat(slashes*2));out.push('"');out
}

/// Request Windows UAC for the fixed sibling daemon. Does not launch a shell or PowerShell.
#[cfg(windows)]
pub fn launch_daemon(executable:&std::path::Path,data:&std::path::Path,sid:&str,elevated:bool)->Result<u32> {
    use windows_sys::Win32::{Foundation::CloseHandle,System::Threading::GetProcessId,UI::{Shell::{ShellExecuteExW,SHELLEXECUTEINFOW,SEE_MASK_NOCLOSEPROCESS,SEE_MASK_NOASYNC,SEE_MASK_FLAG_NO_UI},WindowsAndMessaging::SW_HIDE}};
    anyhow::ensure!(executable.file_name()==Some(std::ffi::OsStr::new("pcscope-service.exe")) && executable.is_file(),"后台程序不存在或不是指定程序");
    anyhow::ensure!(sid==user_sid()?,"原账户 SID 不匹配");
    let wide=|s:&str|s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let file=wide(&executable.to_string_lossy());let verb=wide(if elevated{"runas"}else{"open"});
    let parameters=wide(&format!("{} --replace {}",windows_argument(&data.to_string_lossy()),windows_argument(sid)));
    unsafe {
        let mut launch:SHELLEXECUTEINFOW=std::mem::zeroed();launch.cbSize=std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;launch.fMask=SEE_MASK_NOCLOSEPROCESS|SEE_MASK_NOASYNC|SEE_MASK_FLAG_NO_UI;launch.lpVerb=verb.as_ptr();launch.lpFile=file.as_ptr();launch.lpParameters=parameters.as_ptr();launch.nShow=SW_HIDE;
        if ShellExecuteExW(&mut launch)==0 {return Err(std::io::Error::last_os_error().into());}
        anyhow::ensure!(!launch.hProcess.is_null(),"Windows 未返回后台进程句柄");
        let pid=GetProcessId(launch.hProcess);let error=std::io::Error::last_os_error();CloseHandle(launch.hProcess);
        if pid==0 {return Err(error.into());}Ok(pid)
    }
}

#[cfg(windows)]
pub fn elevated() -> Result<bool> {
    use windows_sys::Win32::{Foundation::CloseHandle, Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY}, System::Threading::{GetCurrentProcess, OpenProcessToken}};
    unsafe {
        let mut token=std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(),TOKEN_QUERY,&mut token)==0 {return Err(std::io::Error::last_os_error().into());}
        let mut elevation=TOKEN_ELEVATION {TokenIsElevated:0}; let mut len=0;
        let ok=GetTokenInformation(token,TokenElevation,(&mut elevation as *mut TOKEN_ELEVATION).cast(),std::mem::size_of::<TOKEN_ELEVATION>() as u32,&mut len);
        let error=std::io::Error::last_os_error(); CloseHandle(token);
        if ok==0 {return Err(error.into());} Ok(elevation.TokenIsElevated!=0)
    }
}

#[cfg(windows)]
pub fn user_sid() -> Result<String> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, LocalFree},
        Security::{
            Authorization::ConvertSidToStringSidW, GetTokenInformation, TokenUser, TOKEN_QUERY,
            TOKEN_USER,
        },
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    };
    unsafe {
        let mut token = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let mut bytes = 0;
        GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut bytes);
        let mut buffer = vec![0usize; (bytes as usize).div_ceil(std::mem::size_of::<usize>())];
        let ok = GetTokenInformation(
            token,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            bytes,
            &mut bytes,
        );
        let error = std::io::Error::last_os_error();
        CloseHandle(token);
        if ok == 0 {
            return Err(error.into());
        }
        let user = &*(buffer.as_ptr() as *const TOKEN_USER);
        let mut text = std::ptr::null_mut();
        if ConvertSidToStringSidW(user.User.Sid, &mut text) == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let mut len = 0;
        while *text.add(len) != 0 {
            len += 1;
        }
        let sid = String::from_utf16_lossy(std::slice::from_raw_parts(text, len));
        LocalFree(text.cast());
        Ok(sid)
    }
}

#[cfg(windows)]
pub fn server(endpoint: &Endpoint, first: bool) -> Result<tokio::net::windows::named_pipe::NamedPipeServer> {
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::{
            Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW,
            SECURITY_ATTRIBUTES,
        },
    };
    let sddl: Vec<u16> = format!("D:P(A;;GA;;;SY)(A;;GA;;;{})", user_sid()?)
        .encode_utf16()
        .chain(Some(0))
        .collect();
    unsafe {
        let mut descriptor = std::ptr::null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &mut descriptor,
            std::ptr::null_mut(),
        ) == 0
        {
            return Err(std::io::Error::last_os_error().into());
        }
        let mut attrs = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor,
            bInheritHandle: 0,
        };
        let result = tokio::net::windows::named_pipe::ServerOptions::new()
            .first_pipe_instance(first)
            .reject_remote_clients(true)
            .create_with_security_attributes_raw(
                endpoint.pipe_name(),
                (&mut attrs as *mut SECURITY_ATTRIBUTES).cast(),
            );
        LocalFree(descriptor);
        Ok(result?)
    }
}

#[cfg(windows)]
pub async fn call(request: Request) -> Result<Value> {
    let data = std::env::var_os("PCSCOPE_DATA_DIR")
        .ok_or_else(|| anyhow::anyhow!("PCSCOPE_DATA_DIR is required; select the target daemon explicitly"))?;
    let endpoint = Endpoint::for_directory(std::path::Path::new(&data))?;
    call_on(&endpoint, request).await
}

#[cfg(windows)]
pub async fn call_on(endpoint: &Endpoint, request: Request) -> Result<Value> {
    let operation = async {
        let name = endpoint.pipe_name();
        let mut client = loop {
            match tokio::net::windows::named_pipe::ClientOptions::new().open(name) {
                Ok(c) => break c,
                Err(e) if e.raw_os_error() == Some(231) => {
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await
                }
                Err(e) => return Err(e).context("opening selected backend pipe"),
            }
        };
        let mut message = serde_json::to_vec(&request)?;
        message.push(b'\n');
        client.write_all(&message).await?;
        let mut bytes = Vec::new();
        BufReader::new(client)
            .take((MAX_RESPONSE + 1) as u64)
            .read_until(b'\n', &mut bytes)
            .await?;
        if bytes.len() > MAX_RESPONSE {
            bail!("IPC response exceeds limit");
        }
        let response: Value = serde_json::from_slice(&bytes)?;
        if response["ok"] != true {
            bail!(
                "{}",
                response["error"]
                    .as_str()
                    .unwrap_or("service request failed")
            );
        }
        Ok(response["data"].clone())
    };
    tokio::time::timeout(std::time::Duration::from_secs(10), operation).await?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[tokio::test]
    async fn separate_directories_and_aliases_use_the_correct_pipe() -> Result<()> {
        let root=std::env::temp_dir().join(format!("pcscope-ipc-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()));
        std::fs::create_dir_all(root.join("甲"))?;
        std::fs::create_dir_all(root.join("乙"))?;
        let a=Endpoint::for_directory(&root.join("甲"))?;
        let alias=Endpoint::for_directory(&root.join("甲").join("."))?;
        let b=Endpoint::for_directory(&root.join("乙"))?;
        assert_eq!(a.instance_id,alias.instance_id);
        assert_ne!(a.instance_id,b.instance_id);
        assert_ne!(a.pipe_name(),b.pipe_name());
        let first=server(&a,true)?;
        assert!(server(&alias,true).is_err(),"same directory alias bypassed duplicate protection");
        let second=server(&b,true)?;
        let reply=|pipe:tokio::net::windows::named_pipe::NamedPipeServer,label:&'static str|tokio::spawn(async move {
            pipe.connect().await?;
            let mut reader=BufReader::new(pipe);
            let mut request=String::new();reader.read_line(&mut request).await?;
            assert!(matches!(serde_json::from_str::<Request>(&request)?,Request::Status));
            let bytes=serde_json::to_vec(&serde_json::json!({"ok":true,"data":{"label":label}}))?;
            reader.get_mut().write_all(&bytes).await?;reader.get_mut().write_all(b"\n").await?;
            Ok::<_,anyhow::Error>(())
        });
        let first=reply(first,"甲");let second=reply(second,"乙");
        let (one,two)=tokio::join!(call_on(&alias,Request::Status),call_on(&b,Request::Status));
        assert_eq!(one?["label"],"甲");assert_eq!(two?["label"],"乙");
        first.await??;second.await??;
        std::fs::remove_dir(root.join("甲"))?;std::fs::remove_dir(root.join("乙"))?;std::fs::remove_dir(root)?;
        Ok(())
    }
    #[test]fn launch_parameters_do_not_turn_paths_into_commands(){assert_eq!(windows_argument("C:\\space dir\\"),"\"C:\\space dir\\\\\"");assert_eq!(windows_argument("a\"b"),"\"a\\\"b\"");assert_eq!(windows_argument("x; $value"),"\"x; $value\"");}

    #[cfg(windows)]
    #[tokio::test]
    async fn pipe_preserves_account_acl_and_medium_or_unlabelled_integrity() -> Result<()> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::{Foundation::LocalFree,Security::{GetKernelObjectSecurity,DACL_SECURITY_INFORMATION,LABEL_SECURITY_INFORMATION,Authorization::ConvertSecurityDescriptorToStringSecurityDescriptorW}};
        let root=std::env::temp_dir().join(format!("pcscope-pipe-label-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()));
        std::fs::create_dir(&root)?;
        let endpoint=Endpoint::for_directory(&root)?;let pipe=server(&endpoint,true)?;
        let information=DACL_SECURITY_INFORMATION|LABEL_SECURITY_INFORMATION;
        let mut size=0;
        unsafe{GetKernelObjectSecurity(pipe.as_raw_handle(),information,std::ptr::null_mut(),0,&mut size);}
        anyhow::ensure!(size>0,"pipe label could not be read");
        let mut bytes=vec![0u32;(size as usize).div_ceil(4)];
        unsafe{
            anyhow::ensure!(GetKernelObjectSecurity(pipe.as_raw_handle(),information,bytes.as_mut_ptr().cast(),size,&mut size)!=0,"{}",std::io::Error::last_os_error());
            let mut sddl=std::ptr::null_mut();
            anyhow::ensure!(ConvertSecurityDescriptorToStringSecurityDescriptorW(bytes.as_mut_ptr().cast(),1,information,&mut sddl,std::ptr::null_mut())!=0,"{}",std::io::Error::last_os_error());
            let mut len=0;while *sddl.add(len)!=0{len+=1;}
            let text=String::from_utf16_lossy(std::slice::from_raw_parts(sddl,len));LocalFree(sddl.cast());
            let expected=format!("D:P(A;;FA;;;SY)(A;;FA;;;{})",user_sid()?);
            // Windows treats an unlabelled object as medium integrity. Both representations
            // must keep exactly the SYSTEM + original-account protected DACL; high/low labels
            // and any broad/foreign-account ACE still fail this check.
            assert!(text==expected||text==format!("{expected}S:(ML;;NW;;;ME)"),"unexpected actual pipe security: {text}");
        }
        drop(pipe);std::fs::remove_dir(root)?;Ok(())
    }

    /// The wire carries the contract's own request type, so a request the query layer understands
    /// cannot fail to survive the trip. This checks the tag and the nested shape together, because
    /// `operation` names the request and `query` names which of the seven it is.
    #[test]
    fn a_query_request_round_trips_with_its_nested_shape() {
        let line = r#"{"operation":"query","request":{"query":"instance_list","limit":25,"sort":"name_asc"}}"#;
        let message: Request = serde_json::from_str(line).expect("a query request parses");
        let Request::Query { request } = &message else { panic!("expected a query request") };
        let pcscope_contracts::query::QueryRequest::InstanceList(filter) = request else { panic!("expected an instance list") };
        assert_eq!(filter.limit, 25);
        let encoded = serde_json::to_string(&message).unwrap();
        assert!(encoded.contains("\"query\":\"instance_list\""), "{encoded}");
        assert!(encoded.contains("\"operation\":\"query\""), "{encoded}");
    }

    #[test]
    fn an_unknown_operation_is_refused_rather_than_ignored() {
        // `deny_unknown_fields` is what keeps a typo in an operation name from being read as a
        // request that simply carried no arguments.
        let line = r#"{"operation":"instances_list"}"#;
        assert!(serde_json::from_str::<Request>(line).is_err());
    }
    #[test]
    fn schedule_wire_accepts_legacy_full_updates_and_new_field_patches() {
        for line in [r#"{"operation":"set_plugin","id":"network","enabled":false,"seconds":60}"#,
            r#"{"operation":"set_plugin","id":"network","enabled":false}"#,
            r#"{"operation":"set_plugin","id":"network","seconds":60}"#,
            r#"{"operation":"set_global_collection_schedule","seconds":60}"#,
            r#"{"operation":"global_collection_schedule"}"#] {
            let request:Request=serde_json::from_str(line).unwrap();
            serde_json::from_str::<Request>(&serde_json::to_string(&request).unwrap()).unwrap();
        }
        for value in ["-1","1.5","null","\"60\""] {
            assert!(serde_json::from_str::<Request>(&format!("{{\"operation\":\"set_global_collection_schedule\",\"seconds\":{value}}}")).is_err());
        }
    }
}
