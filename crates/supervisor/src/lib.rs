use anyhow::{bail, Context, Result};
use pcscope_contracts::{Envelope, MAX_LINE_BYTES};
use std::{
    io::{BufRead, BufReader, Read, Write},
    path::Path,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

pub struct Collection {
    pub messages: Vec<Envelope>,
    pub diagnostic: String,
}

#[cfg(all(test,windows))] mod tests {
    use super::*;
    #[test] fn native_job_observes_root_and_descendant_with_creation_identity() {
        let powershell=std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let script=r#"Start-Process -FilePath "$PSHOME\powershell.exe" -ArgumentList '-NoProfile -NonInteractive -Command Start-Sleep -Seconds 1' -WindowStyle Hidden -Wait; '{"protocol_version":1,"request_id":"native-test","type":"gap","source":"test","reason":"test","observed_at":"2026-09-11T00:00:00Z"}'"#;
        let args=["-NoProfile","-NonInteractive","-Command",script].map(std::ffi::OsString::from);
        let mut observed=Vec::new();
        let result=collect_observed(&powershell,&args,None,Duration::from_secs(15),&mut |p|{observed.extend(p);Ok(())}).unwrap();
        assert_eq!(result.messages.len(),1);
        assert!(observed.iter().any(|p|p.is_root));assert!(observed.iter().any(|p|!p.is_root));
        for p in observed{assert!(p.pid>0);assert!(p.created_ms<=p.seen_ms);assert!(std::path::Path::new(&p.image).is_absolute());}
    }
}

/// Run a trusted, one-shot collector. No shell or arbitrary command arguments.
/// Production streaming/subscribe handshake is a separate runtime mode.
pub fn collect(executable: &Path, timeout: Duration) -> Result<Collection> {
    collect_with_args(executable, &[], timeout)
}

pub fn collect_with_args(
    executable: &Path,
    args: &[std::ffi::OsString],
    timeout: Duration,
) -> Result<Collection> {
    collect_with_input(executable, args, None, timeout)
}

pub fn collect_with_input(
    executable: &Path,
    args: &[std::ffi::OsString],
    input: Option<Vec<u8>>,
    timeout: Duration,
) -> Result<Collection> {
    collect_observed(executable,args,input,timeout,&mut |_|Ok(()))
}

#[derive(Debug,Clone)]
pub struct ProcessIdentity {pub is_root:bool,pub pid:u32,pub created_ms:i64,pub seen_ms:i64,pub ended_ms:Option<i64>,pub image:String}
pub fn collect_observed(executable:&Path,args:&[std::ffi::OsString],input:Option<Vec<u8>>,timeout:Duration,observer:&mut dyn FnMut(Vec<ProcessIdentity>)->Result<()>)->Result<Collection>{
    if input.as_ref().is_some_and(|v| v.len() > 65536) {
        bail!("collector request exceeds limit");
    }
    let executable = executable
        .canonicalize()
        .context("collector executable not found")?;
    let mut command = Command::new(&executable);
    command.args(args);
    #[cfg(windows)]
    if executable.file_name().is_some_and(|name|name.eq_ignore_ascii_case("powershell.exe")) {
        // Do not inherit PS7 modules or user-writable modules into a privileged PS5 fallback.
        if let Some(windows)=std::env::var_os("SystemRoot") {command.env("PSModulePath",std::path::PathBuf::from(windows).join("System32/WindowsPowerShell/v1.0/Modules"));}
        command.env_remove("PSExecutionPolicyPreference");
    }
    command
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let mut child = command.spawn().context("cannot start collector")?;
    #[cfg(windows)]
    let job = match job::Job::attach(&child) {
        Ok(v) => v,
        Err(e) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(e);
        }
    };
    #[cfg(windows)] observer(job.identities()?)?;
    let writer = if let Some(bytes) = input {
        let mut pipe = child.stdin.take().context("missing stdin")?;
        Some(thread::spawn(move || pipe.write_all(&bytes)))
    } else {
        None
    };
    let stdout = child.stdout.take().context("missing stdout")?;
    let stderr = child.stderr.take().context("missing stderr")?;
    let (send, receive) = mpsc::channel();
    let reader = thread::spawn(move || {
        let result = (|| -> Result<Vec<Envelope>> {
            let mut input = BufReader::new(stdout);
            let mut messages = Vec::new();
            let mut total = 0;
            loop {
                let mut bytes = Vec::new();
                let n = Read::by_ref(&mut input)
                    .take((MAX_LINE_BYTES + 1) as u64)
                    .read_until(b'\n', &mut bytes)?;
                if n == 0 {
                    break;
                }
                total += n;
                if n > MAX_LINE_BYTES || total > 64 * MAX_LINE_BYTES {
                    bail!("collector output budget exceeded");
                }
                messages.push(pcscope_contracts::parse(std::str::from_utf8(&bytes)?)?);
            }
            Ok(messages)
        })();
        let _ = send.send(result);
    });
    let diagnostics = thread::spawn(move || {
        let mut input = BufReader::new(stderr);
        let mut kept = Vec::new();
        let mut block = [0; 4096];
        loop {
            match input.read(&mut block) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let amount = n.min(65536usize.saturating_sub(kept.len()));
                    kept.extend_from_slice(&block[..amount]);
                }
            }
        }
        String::from_utf8_lossy(&kept).into_owned()
    });
    let started = Instant::now();
    let mut output = None;
    let mut sampled=Instant::now();
    let outcome = loop {
        #[cfg(windows)] if sampled.elapsed()>=Duration::from_millis(200){
            match job.identities().and_then(|identities|observer(identities)){Ok(())=>{},Err(e)=>break Err(e)}
            sampled=Instant::now();
        }
        if output.is_none() {
            if let Ok(result) = receive.try_recv() {
                output = Some(result);
            }
        }
        if matches!(&output, Some(Err(_))) {
            break Err(anyhow::anyhow!("collector emitted invalid protocol"));
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                break if status.success() {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!("collector exited with {status}"))
                }
            }
            Err(e) => break Err(e.into()),
            Ok(None) => {}
        }
        if started.elapsed() >= timeout {
            break Err(anyhow::anyhow!("collector timed out"));
        }
        thread::sleep(Duration::from_millis(20));
    };
    let _ = child.kill();
    let _ = child.wait();
    #[cfg(windows)] let observation=if let Ok(mut identity)=process_identity(child.id()){identity.is_root=true;observer(vec![identity])}else{Ok(())};
    #[cfg(windows)]
    drop(job); // Close descendant pipe handles before joining readers.
    let _ = reader.join();
    if let Some(writer) = writer {
        writer
            .join()
            .map_err(|_| anyhow::anyhow!("collector stdin writer failed"))??;
    }
    let diagnostic = diagnostics.join().unwrap_or_default();
    #[cfg(windows)] observation?;
    if std::env::var_os("PCSCOPE_DEBUG_DIAGNOSTICS").is_some() {
        eprintln!("{diagnostic}");
    }
    outcome.with_context(|| {
        format!(
            "collector failed; diagnostic bytes captured: {}",
            diagnostic.len()
        )
    })?;
    let messages = match output {
        Some(result) => result?,
        None => receive.recv().context("collector reader stopped")??,
    };
    if messages.is_empty() {
        bail!("collector exited without protocol messages");
    }
    Ok(Collection {
        messages,
        diagnostic,
    })
}

#[cfg(windows)]
mod job {
    use super::*;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::JobObjects::*,
    };
    pub struct Job(HANDLE,u32);
    impl Job {
        pub fn identities(&self)->Result<Vec<ProcessIdentity>> {
            let mut buffer=vec![0usize;512];
            loop {unsafe {
                if QueryInformationJobObject(self.0,JobObjectBasicProcessIdList,buffer.as_mut_ptr().cast(),(buffer.len()*std::mem::size_of::<usize>()) as u32,std::ptr::null_mut())!=0 {
                    let bytes=buffer.as_ptr().cast::<u8>();let count=*(bytes.add(4).cast::<u32>()) as usize;
                    if count>(buffer.len()*std::mem::size_of::<usize>()-8)/std::mem::size_of::<usize>(){bail!("invalid job process list");}
                    let pids=std::slice::from_raw_parts(bytes.add(8).cast::<usize>(),count);
                    return Ok(pids.iter().filter_map(|pid|process_identity(*pid as u32).ok()).map(|mut p|{p.is_root=p.pid==self.1;p}).collect());
                }
                let error=std::io::Error::last_os_error();if error.raw_os_error()!=Some(234)||buffer.len()>=131072{return Err(error.into());}
                buffer.resize(buffer.len()*2,0);
            }}
        }
        pub fn attach(child: &std::process::Child) -> Result<Self> {
            unsafe {
                let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if handle.is_null() {
                    return Err(std::io::Error::last_os_error().into());
                }
                let job = Self(handle,child.id());
                let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                if SetInformationJobObject(
                    handle,
                    JobObjectExtendedLimitInformation,
                    &limits as *const _ as *const _,
                    std::mem::size_of_val(&limits) as u32,
                ) == 0
                    || AssignProcessToJobObject(handle, child.as_raw_handle()) == 0
                {
                    return Err(std::io::Error::last_os_error().into());
                }
                Ok(job)
            }
        }
    }
    impl Drop for Job {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

#[cfg(windows)]
pub fn process_identity(pid:u32)->Result<ProcessIdentity>{
 use windows_sys::Win32::{Foundation::{CloseHandle,FILETIME},System::Threading::{OpenProcess,GetProcessTimes,QueryFullProcessImageNameW,PROCESS_QUERY_LIMITED_INFORMATION}};
 unsafe {
  let handle=OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION,0,pid);if handle.is_null(){return Err(std::io::Error::last_os_error().into());}
  struct Guard(windows_sys::Win32::Foundation::HANDLE);impl Drop for Guard{fn drop(&mut self){unsafe{CloseHandle(self.0);}}}let _guard=Guard(handle);
  let mut created:FILETIME=std::mem::zeroed();let mut ended=created;let mut kernel=created;let mut user=created;
  if GetProcessTimes(handle,&mut created,&mut ended,&mut kernel,&mut user)==0{return Err(std::io::Error::last_os_error().into());}
  let ticks=|t:FILETIME|((t.dwHighDateTime as u64)<<32)|t.dwLowDateTime as u64;
  let millis=|t:FILETIME|(ticks(t)/10000) as i64-11644473600000;
  let mut image=vec![0u16;32768];let mut size=image.len() as u32;
  if QueryFullProcessImageNameW(handle,0,image.as_mut_ptr(),&mut size)==0{return Err(std::io::Error::last_os_error().into());}
  let now=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_millis() as i64;
  Ok(ProcessIdentity{is_root:false,pid,created_ms:millis(created),seen_ms:now,ended_ms:if ticks(ended)==0{None}else{Some(millis(ended))},image:String::from_utf16_lossy(&image[..size as usize])})
 }
}
