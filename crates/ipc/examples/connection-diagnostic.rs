use anyhow::Result;
#[tokio::main]
async fn main()->Result<()> {
    let directory=std::env::var_os("PCSCOPE_DATA_DIR").ok_or_else(||anyhow::anyhow!("PCSCOPE_DATA_DIR required"))?;
    let endpoint=pcscope_ipc::Endpoint::for_directory(std::path::Path::new(&directory))?;
    println!("account: {}",pcscope_ipc::user_sid()?);
    println!("pipe: {}",endpoint.pipe_name());
    unsafe {
        use windows_sys::Win32::{Foundation::{CloseHandle,LocalFree},Security::{GetTokenInformation,TokenRestrictedSids,TokenIntegrityLevel,TOKEN_MANDATORY_LABEL,TOKEN_QUERY,TOKEN_GROUPS,Authorization::ConvertSidToStringSidW},System::Threading::{GetCurrentProcess,OpenProcessToken}};
        let mut token=std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(),TOKEN_QUERY,&mut token)!=0 {
            let mut size=0;GetTokenInformation(token,TokenRestrictedSids,std::ptr::null_mut(),0,&mut size);
            let mut bytes=vec![0usize;(size as usize).div_ceil(std::mem::size_of::<usize>())];
            if GetTokenInformation(token,TokenRestrictedSids,bytes.as_mut_ptr().cast(),size,&mut size)!=0 {
                let groups=&*(bytes.as_ptr() as *const TOKEN_GROUPS);
                println!("restricted SID count: {}",groups.GroupCount);
                for i in 0..groups.GroupCount.min(32) as usize {
                    let group=groups.Groups.as_ptr().add(i);let mut text=std::ptr::null_mut();
                    if ConvertSidToStringSidW((*group).Sid,&mut text)!=0{let mut len=0;while *text.add(len)!=0{len+=1;}println!("restricted: {}",String::from_utf16_lossy(std::slice::from_raw_parts(text,len)));LocalFree(text.cast());}
                }
            }
            size=0;GetTokenInformation(token,TokenIntegrityLevel,std::ptr::null_mut(),0,&mut size);
            let mut bytes=vec![0usize;(size as usize).div_ceil(std::mem::size_of::<usize>())];
            if GetTokenInformation(token,TokenIntegrityLevel,bytes.as_mut_ptr().cast(),size,&mut size)!=0 {
                let label=&*(bytes.as_ptr() as *const TOKEN_MANDATORY_LABEL);let mut text=std::ptr::null_mut();
                if ConvertSidToStringSidW(label.Label.Sid,&mut text)!=0{let mut len=0;while *text.add(len)!=0{len+=1;}println!("integrity: {}",String::from_utf16_lossy(std::slice::from_raw_parts(text,len)));LocalFree(text.cast());}
            }
            CloseHandle(token);
        }
    }
    use windows_sys::Win32::{Foundation::INVALID_HANDLE_VALUE,Storage::FileSystem::{CreateFileW,OPEN_EXISTING,FILE_FLAG_OVERLAPPED,SECURITY_SQOS_PRESENT,SECURITY_IDENTIFICATION}};
    let name:Vec<u16>=endpoint.pipe_name().encode_utf16().chain(Some(0)).collect();
    let handle=unsafe{CreateFileW(name.as_ptr(),0xc0000000,0,std::ptr::null(),OPEN_EXISTING,FILE_FLAG_OVERLAPPED|SECURITY_SQOS_PRESENT|SECURITY_IDENTIFICATION,std::ptr::null_mut())};
    if handle==INVALID_HANDLE_VALUE{println!("CreateFile: {}",std::io::Error::last_os_error());}
    else {
        println!("CreateFile: success");
        use std::os::windows::io::RawHandle;
        match unsafe{tokio::net::windows::named_pipe::NamedPipeClient::from_raw_handle(handle as RawHandle)} {
            Ok(client)=>{println!("IO registration: success");drop(client);},
            Err(error)=>println!("IO registration: {error}"),
        }
    }
    match pcscope_ipc::call_on(&endpoint,pcscope_ipc::Request::MonitoringSettings).await {
        Ok(value)=>println!("query: pid={} elevated={}",value["pid"],value["elevated"]),
        Err(error)=>println!("query: {error:#}"),
    }
    Ok(())
}
