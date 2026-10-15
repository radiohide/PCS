//! Bounded nonrecursive Win32 directory notifications. No last-access notifications.
use anyhow::{ensure,Result};
use std::path::Path;
use windows_sys::Win32::{Foundation::{CloseHandle,HANDLE,INVALID_HANDLE_VALUE,ERROR_IO_INCOMPLETE,ERROR_IO_PENDING},Storage::FileSystem::{CreateFileW,ReadDirectoryChangesW,FILE_LIST_DIRECTORY,FILE_SHARE_READ,FILE_SHARE_WRITE,FILE_SHARE_DELETE,OPEN_EXISTING,FILE_FLAG_BACKUP_SEMANTICS,FILE_FLAG_OVERLAPPED},System::{IO::{CancelIoEx,GetOverlappedResult,OVERLAPPED},Threading::{CreateEventW,ResetEvent}}};

pub const FILTER:u32=0x01|0x04|0x08|0x10|0x100;
pub struct Notification {pub name:String,pub action:u32}
pub struct Watcher {
    handle:HANDLE,event:HANDLE,buffer:Box<[u32]>,overlapped:Box<OVERLAPPED>,pending:bool,
}
impl Watcher {
    pub fn open(path:&Path)->Result<Self> {
        use std::os::windows::fs::MetadataExt;
        let mut parent=Some(path);
        while let Some(path)=parent {let m=std::fs::symlink_metadata(path)?;ensure!(m.file_attributes()&0x400==0,"directory reparse target not followed");parent=path.parent();}
        ensure!(path.is_dir(),"notification parent is not a directory");
        use std::os::windows::ffi::OsStrExt;
        let wide:Vec<_>=path.as_os_str().encode_wide().chain(Some(0)).collect();
        let handle=unsafe {CreateFileW(wide.as_ptr(),FILE_LIST_DIRECTORY,FILE_SHARE_READ|FILE_SHARE_WRITE|FILE_SHARE_DELETE,std::ptr::null(),OPEN_EXISTING,FILE_FLAG_BACKUP_SEMANTICS|FILE_FLAG_OVERLAPPED,std::ptr::null_mut())};
        if handle==INVALID_HANDLE_VALUE{return Err(std::io::Error::last_os_error().into());}
        let event=unsafe {CreateEventW(std::ptr::null(),1,0,std::ptr::null())};
        if event.is_null(){let error=std::io::Error::last_os_error();unsafe{CloseHandle(handle);}return Err(error.into());}
        let mut watcher=Self{handle,event,buffer:vec![0u32;16000].into_boxed_slice(),overlapped:Box::new(unsafe{std::mem::zeroed()}),pending:false};
        watcher.arm()?;Ok(watcher)
    }
    fn arm(&mut self)->Result<()> {
        unsafe {
            ResetEvent(self.event);*self.overlapped=std::mem::zeroed();self.overlapped.hEvent=self.event;
            let ok=ReadDirectoryChangesW(self.handle,self.buffer.as_mut_ptr().cast(),(self.buffer.len()*4) as u32,0,FILTER,std::ptr::null_mut(),self.overlapped.as_mut(),None);
            if ok==0&&std::io::Error::last_os_error().raw_os_error()!=Some(ERROR_IO_PENDING as i32){return Err(std::io::Error::last_os_error().into());}
            self.pending=true;Ok(())
        }
    }
    /// None = still waiting. Empty vector = notification overflow: rescan known files.
    pub fn poll(&mut self)->Result<Option<Vec<Notification>>> {
        let mut bytes=0;
        if unsafe{GetOverlappedResult(self.handle,self.overlapped.as_ref(),&mut bytes,0)}==0 {
            let error=std::io::Error::last_os_error();
            if error.raw_os_error()==Some(ERROR_IO_INCOMPLETE as i32){return Ok(None);}
            self.pending=false;return Err(error.into());
        }
        self.pending=false;
        ensure!(bytes as usize<=self.buffer.len()*4,"notification result exceeds allocated buffer");
        let material=unsafe{std::slice::from_raw_parts(self.buffer.as_ptr().cast::<u8>(),bytes as usize)};
        let notifications=parse(material);self.arm()?;Ok(Some(notifications?))
    }
}
impl Drop for Watcher {
    fn drop(&mut self) {
        unsafe {
            if self.pending {
                CancelIoEx(self.handle,self.overlapped.as_ref());
                let mut bytes=0;GetOverlappedResult(self.handle,self.overlapped.as_ref(),&mut bytes,1);
            }
            CloseHandle(self.handle);CloseHandle(self.event);
        }
    }
}
fn parse(bytes:&[u8])->Result<Vec<Notification>> {
    let mut rows=Vec::new();let mut offset=0;
    while offset<bytes.len() {
        ensure!(bytes.len()-offset>=12,"truncated notification header");
        let word=|i|u32::from_le_bytes(bytes[offset+i..offset+i+4].try_into().unwrap());
        let next=word(0) as usize;let action=word(4);let length=word(8) as usize;
        ensure!(length%2==0&&length<=bytes.len()-offset-12,"truncated notification name");
        let wide:Vec<_>=bytes[offset+12..offset+12+length].chunks_exact(2).map(|c|u16::from_le_bytes([c[0],c[1]])).collect();
        let name=String::from_utf16(&wide)?;
        ensure!(!name.contains(['\\','/','\0'])&&name!="."&&name!="..","notification escaped nonrecursive scope");
        rows.push(Notification{name,action});
        if next==0 {break;}
        ensure!(next>=12+length&&next%4==0&&next<=bytes.len()-offset,"invalid notification offset");offset+=next;
    }
    Ok(rows)
}
#[cfg(test)]mod tests {
    use super::*;
    #[test]fn notifications_are_bounded_and_do_not_track_access_time(){assert_eq!(FILTER&0x20,0);assert!(parse(&[0;11]).is_err());let mut bytes=vec![0u8;14];bytes[4..8].copy_from_slice(&3u32.to_le_bytes());bytes[8..12].copy_from_slice(&2u32.to_le_bytes());bytes[12..14].copy_from_slice(&(b'x' as u16).to_le_bytes());let rows=parse(&bytes).unwrap();assert_eq!(rows[0].name,"x");assert_eq!(rows[0].action,3);bytes[8..12].copy_from_slice(&8u32.to_le_bytes());assert!(parse(&bytes).is_err());}
    #[test]fn native_notification_reports_known_file_write_and_cancels_cleanly()->Result<()> {
        let root=std::env::temp_dir().join(format!("pcscope-notify-{}-{}",std::process::id(),chrono::Utc::now().timestamp_nanos_opt().unwrap()));std::fs::create_dir(&root)?;
        let mut watcher=Watcher::open(&root)?;std::fs::write(root.join("module.dll"),b"fixture")?;
        let end=std::time::Instant::now()+std::time::Duration::from_secs(5);let mut observed=false;
        while std::time::Instant::now()<end {if let Some(rows)=watcher.poll()?{observed|=rows.iter().any(|r|r.name=="module.dll");if observed{break;}}std::thread::sleep(std::time::Duration::from_millis(20));}
        ensure!(observed,"native write notification absent");drop(watcher);std::fs::remove_file(root.join("module.dll"))?;std::fs::remove_dir(root)?;Ok(())
    }
}
