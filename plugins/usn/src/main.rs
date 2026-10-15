use anyhow::{Context, Result};
use pcscope_contracts::{Envelope, Event, Item, Message};
use serde_json::{json, Value};
use std::io::Read;

fn output(message: Message, request_id: String) -> Result<()> {
    let encoded = serde_json::to_string(&Envelope {
        protocol_version: 1,
        request_id,
        message,
    })?;
    anyhow::ensure!(
        encoded.len() < pcscope_contracts::MAX_LINE_BYTES,
        "USN batch exceeds protocol limit"
    );
    println!("{encoded}");
    Ok(())
}

#[cfg(windows)]
mod native {
    use super::*;
    use windows_sys::Win32::{
        Foundation::{CloseHandle, GENERIC_READ, HANDLE, INVALID_HANDLE_VALUE},
        Storage::FileSystem::{
            CreateFileW, GetDriveTypeW, GetFileInformationByHandle, GetLogicalDrives,
            GetVolumeInformationW, GetVolumeNameForVolumeMountPointW, BY_HANDLE_FILE_INFORMATION,
            FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_FLAG_BACKUP_SEMANTICS,
            OPEN_EXISTING,
        },
        System::IO::DeviceIoControl,
    };
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }
    fn string(b: &[u16]) -> String {
        String::from_utf16_lossy(&b[..b.iter().position(|v| *v == 0).unwrap_or(b.len())])
    }
    struct Volume(HANDLE);
    impl Drop for Volume {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    fn ioctl(handle: HANDLE, code: u32, input: &[u8], size: usize) -> Result<Vec<u8>> {
        let mut buffer = vec![0u8; size];
        let mut returned = 0;
        unsafe {
            if DeviceIoControl(
                handle,
                code,
                if input.is_empty() {
                    std::ptr::null()
                } else {
                    input.as_ptr().cast()
                },
                input.len() as u32,
                buffer.as_mut_ptr().cast(),
                buffer.len() as u32,
                &mut returned,
                std::ptr::null_mut(),
            ) == 0
            {
                return Err(std::io::Error::last_os_error().into());
            }
        };
        buffer.truncate(returned as usize);
        Ok(buffer)
    }
    fn query(handle: HANDLE) -> Result<(u64, i64, i64)> {
        let b = ioctl(handle, 0x900f4, &[], 128)?;
        anyhow::ensure!(b.len() >= 56, "truncated journal metadata");
        let id = u64::from_le_bytes(b[0..8].try_into()?);
        let first = i64::from_le_bytes(b[8..16].try_into()?);
        let next = i64::from_le_bytes(b[16..24].try_into()?);
        let lowest = i64::from_le_bytes(b[24..32].try_into()?);
        Ok((id, first.max(lowest), next))
    }
    fn excluded_ids(paths: &Value, serial: u32) -> Result<std::collections::HashSet<String>> {
        let mut ids = std::collections::HashSet::new();
        if let Some(paths) = paths.as_array() {
            for path in paths {
                let path = wide(path.as_str().context("exclusion path must be a string")?);
                let handle = unsafe {
                    CreateFileW(
                        path.as_ptr(),
                        FILE_READ_ATTRIBUTES,
                        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                        std::ptr::null(),
                        OPEN_EXISTING,
                        0,
                        std::ptr::null_mut(),
                    )
                };
                if handle == INVALID_HANDLE_VALUE {
                    let error = std::io::Error::last_os_error();
                    if error.kind() == std::io::ErrorKind::NotFound {
                        continue;
                    }
                    return Err(error.into());
                }
                let handle = Volume(handle);
                let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
                if unsafe { GetFileInformationByHandle(handle.0, &mut info) } == 0 {
                    return Err(std::io::Error::last_os_error().into());
                }
                if info.dwVolumeSerialNumber == serial {
                    let id = (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow);
                    ids.insert(format!("{id:016x}"));
                    ids.insert(format!("{id:032x}"));
                }
            }
        }
        Ok(ids)
    }
    fn collect(
        drive: &str,
        volume: &str,
        serial: u32,
        exclusions: &Value,
        watches: &[String],
        revision: u64,
        resume: Option<&Value>,
        stamp: &str,
    ) -> Result<()> {
        let path = wide(&format!(r"\\.\{drive}"));
        let handle = unsafe {
            CreateFileW(
                path.as_ptr(),
                GENERIC_READ,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(std::io::Error::last_os_error().into());
        }
        let handle = Volume(handle);
        let excluded = excluded_ids(exclusions, serial)?;
        let mut bindings=Vec::new();
        for path in watches {
            let binding=(|| -> Result<(String,String,String,String)> {
            let file=std::path::Path::new(path);
            let parent=file.parent().context("watch file has no parent")?;
            let name=file.file_name().context("watch file has no name")?.to_string_lossy().to_lowercase();
            let parent_path=wide(&parent.to_string_lossy());
            let parent_handle=unsafe {CreateFileW(parent_path.as_ptr(),FILE_READ_ATTRIBUTES,FILE_SHARE_READ|FILE_SHARE_WRITE|FILE_SHARE_DELETE,std::ptr::null(),OPEN_EXISTING,FILE_FLAG_BACKUP_SEMANTICS,std::ptr::null_mut())};
            if parent_handle==INVALID_HANDLE_VALUE {anyhow::bail!("监测文件的父目录不可读，未扩大采集范围：{path}: {}",std::io::Error::last_os_error());}
            let parent_handle=Volume(parent_handle);
            let mut info:BY_HANDLE_FILE_INFORMATION=unsafe{std::mem::zeroed()};
            if unsafe{GetFileInformationByHandle(parent_handle.0,&mut info)}==0 {return Err(std::io::Error::last_os_error().into());}
            anyhow::ensure!(info.dwVolumeSerialNumber==serial,"监测路径跨卷或经过挂载点：{path}");
            let id=(u64::from(info.nFileIndexHigh)<<32)|u64::from(info.nFileIndexLow);
            Ok((format!("{id:016x}"),format!("{id:032x}"),name,path.clone()))
            })();
            match binding {Ok(binding)=>bindings.push(binding),Err(error)=>output(Message::Gap {source:format!("pcscope.usn.{}",drive[..1].to_lowercase()),observed_at:stamp.into(),reason:format!("指定文件路径不可解析，其余列表项继续监测：{path}: {error}")},format!("watch-gap/{stamp}/{path}"))?};
        }
        let (id, first, source_next) = query(handle.0)?;
        let journal = format!("{id:016x}");
        let source = format!("pcscope.usn.{}", drive[..1].to_lowercase());
        let matching_resume=resume.filter(|r|r["checkpoint"].as_str().and_then(|s|serde_json::from_str::<Value>(s).ok()).is_some_and(|cp|cp["watch_revision"]==revision));
        let mut position = pcscope_usn::resume_position(
            volume,
            &journal,
            first,
            source_next,
            matching_resume,
            &format!("{journal}-{stamp}"),
        )?;
        // A new allowlist establishes a new observation boundary, never replaying an old unrestricted cursor.
        if matching_resume.is_none() {position.start=source_next;}
        for _ in 0..8 {
            let mut input = Vec::new();
            input.extend_from_slice(&position.start.to_le_bytes());
            input.extend_from_slice(&u32::MAX.to_le_bytes());
            input.extend_from_slice(&0u32.to_le_bytes());
            input.extend_from_slice(&0u64.to_le_bytes());
            input.extend_from_slice(&0u64.to_le_bytes());
            input.extend_from_slice(&id.to_le_bytes());
            let buffer = ioctl(handle.0, 0x900bb, &input, 16384)?;
            let (next, records) = pcscope_usn::parse_buffer(&buffer)?;
            anyhow::ensure!(next >= position.start, "source cursor regressed");
            let (after_id, _, after_next) = query(handle.0)?;
            anyhow::ensure!(
                id == after_id && next <= after_next,
                "journal changed during read; batch discarded"
            );
            let mut events = Vec::new();
            if let Some(reason) = position.gap.take() {
                events.push(Event{id:format!("gap-{}",position.sequence),occurred_at:stamp.into(),kind:"coverage.usn_gap".into(),object_id:volume.into(),data:json!({"reason":reason,"volume":volume,"basis":"USN checkpoint validation"})});
            }
            for record in records {
                if record.attributes & 0x10 != 0 {continue;}
                if excluded.contains(&record.file_id) {
                    continue;
                }
                let Some(binding)=bindings.iter().find(|(id64,id128,name,_)|pcscope_usn::matches_watch(&record.parent_id,&record.name,id64,id128,name)) else {continue;};
                let nanos = (i128::from(record.timestamp) - 116444736000000000) * 100;
                let source_time = i64::try_from(nanos)
                    .ok()
                    .map(chrono::DateTime::<chrono::Utc>::from_timestamp_nanos);
                events.push(Event{id:record.usn.to_string(),occurred_at:source_time.map(|t|t.to_rfc3339()).unwrap_or_else(||stamp.into()),kind:"files.usn_changed".into(),object_id:binding.3.to_lowercase(),data:json!({"volume":volume,"drive":drive,"journal_id":journal,"usn":record.usn,"file_id":record.file_id,"parent_id":record.parent_id,"name":record.name,"watch_path":binding.3,"watch_revision":revision,"reason_mask":record.reason,"attributes":record.attributes,"record_version":record.version,"source_time_available":source_time.is_some(),"full_path_resolved":false,"basis":"explicit file allowlist matched by current parent directory ID and exact name; parent renames can limit historical path attribution; not each write operation or content version"})});
            }
            let mut checkpoint:Value=serde_json::from_str(&pcscope_usn::checkpoint(volume,&journal,next,after_next))?;
            checkpoint["watch_revision"]=json!(revision);
            output(
                Message::EventBatch {
                    source: source.clone(),
                    epoch: position.epoch.clone(),
                    batch_seq: position.sequence,
                    checkpoint: checkpoint.to_string(),
                    events,
                },
                format!("{source}-{stamp}-{}", position.sequence),
            )?;
            position.sequence += 1;
            position.start = next;
            if next >= after_next {
                break;
            }
        }
        Ok(())
    }
    pub fn run(request: Value) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let watches:Vec<String>=serde_json::from_value(request["watch"]["files"].as_array().map(|a|json!(a)).unwrap_or(json!([])))?;
        let revision=request["watch"]["revision"].as_u64().unwrap_or(0);
        if watches.is_empty() {
            return output(Message::Snapshot {source:"pcscope.usn.watch-list".into(),scope:"explicit-files/v1".into(),run_id:now.clone(),observed_at:now.clone(),complete:true,items:vec![Item{id:"configuration".into(),data:json!({"files":[],"revision":revision,"active":false,"reason":"empty allowlist; no file changes collected"})}]},format!("watch/{now}"));
        }
        output(Message::Snapshot {source:"pcscope.usn.watch-list".into(),scope:"explicit-files/v1".into(),run_id:now.clone(),observed_at:now.clone(),complete:true,items:vec![Item{id:"configuration".into(),data:json!({"files":watches,"revision":revision,"active":true,"reason":"explicit file allowlist; journal readability reported per volume"})}]},format!("watch/{now}"))?;
        let mut volumes = Vec::new();
        let drives = unsafe { GetLogicalDrives() };
        anyhow::ensure!(drives != 0, "cannot enumerate local drives");
        for index in 0..26 {
            if drives & (1 << index) == 0 {
                continue;
            }
            let drive = format!("{}:", (b'A' + index as u8) as char);
            let drive_watches:Vec<String>=watches.iter().filter(|p|p.get(..2).is_some_and(|prefix|prefix.eq_ignore_ascii_case(&drive))).cloned().collect();
            if drive_watches.is_empty(){continue;}
            let root = wide(&format!("{drive}\\"));
            if unsafe { GetDriveTypeW(root.as_ptr()) } != 3 {
                continue;
            }
            let mut filesystem = [0u16; 64];
            let mut guid = [0u16; 128];
            let mut serial = 0u32;
            let result = (|| -> Result<(String, String)> {
                unsafe {
                    if GetVolumeInformationW(
                        root.as_ptr(),
                        std::ptr::null_mut(),
                        0,
                        &mut serial,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        filesystem.as_mut_ptr(),
                        filesystem.len() as u32,
                    ) == 0
                        || GetVolumeNameForVolumeMountPointW(
                            root.as_ptr(),
                            guid.as_mut_ptr(),
                            guid.len() as u32,
                        ) == 0
                    {
                        return Err(std::io::Error::last_os_error().into());
                    }
                }
                Ok((string(&filesystem), string(&guid)))
            })();
            let source = format!("pcscope.usn.{}", drive[..1].to_lowercase());
            match result {
                Ok((fs, volume)) => {
                    volumes.push(Item{id:drive.clone(),data:json!({"volume":volume,"filesystem":fs,"eligible":fs=="NTFS","scope":"fixed drives with drive letters only"})});
                    if fs == "NTFS" && !request["selection"]["disabled_items"].as_array().is_some_and(|a|a.contains(&json!("pcscope.usn.changes"))) {
                        if let Err(error) = collect(
                            &drive,
                            &volume,
                            serial,
                            &request["exclude_files"],
                            &drive_watches,
                            revision,
                            request["resumes"].get(&source),
                            &now,
                        ) {
                            output(
                                Message::Gap {
                                    source: source.clone(),
                                    reason: format!(
                                        "USN unavailable; journal not changed by PCScope: {error}"
                                    ),
                                    observed_at: now.clone(),
                                },
                                format!("{source}/{now}/error"),
                            )?;
                        }
                    }
                }
                Err(error) => {
                    output(
                        Message::Gap {
                            source: source.clone(),
                            reason: format!("Volume identity unavailable: {error}"),
                            observed_at: now.clone(),
                        },
                        format!("{source}/{now}/identity-error"),
                    )?;
                    volumes.push(Item {
                        id: drive,
                        data: json!({"readable":false,"error":error.to_string()}),
                    });
                }
            }
        }
        output(
            Message::Snapshot {
                source: "pcscope.usn.volumes".into(),
                scope: "fixed-drive-letters/v1".into(),
                run_id: now.clone(),
                observed_at: now.clone(),
                complete: true,
                items: volumes,
            },
            format!("volumes/{now}"),
        )
    }
}
fn main() -> Result<()> {
    let mut bytes = Vec::new();
    std::io::stdin().take(65537).read_to_end(&mut bytes)?;
    anyhow::ensure!(bytes.len() <= 65536, "USN resume request exceeds limit");
    let request: Value = serde_json::from_slice(&bytes).context("USN request must be JSON")?;
    #[cfg(windows)]
    {
        native::run(request)
    }
    #[cfg(not(windows))]
    {
        let _ = request;
        anyhow::bail!("Windows required")
    }
}
