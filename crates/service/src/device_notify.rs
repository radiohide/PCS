//! Native PnP notifications only invalidate state; the existing inventory lane performs all reads.
use anyhow::{ensure,Result};
use std::{sync::atomic::{AtomicBool,Ordering},time::{Duration,Instant}};
use windows_sys::Win32::Devices::DeviceAndDriverInstallation::*;
use windows_sys::Win32::{Foundation::HANDLE,NetworkManagement::IpHelper::*,Networking::WinSock::AF_UNSPEC};

pub struct Watcher {handle:HCMNOTIFICATION,dirty:Box<AtomicBool>}
// The registration can move between threads; callback context remains at a stable heap address.
// Unregister waits for callbacks before the context is freed. There is no shared handle access.
unsafe impl Send for Watcher {}
unsafe extern "system" fn callback(_:HCMNOTIFICATION,context:*const core::ffi::c_void,_:CM_NOTIFY_ACTION,_:*const CM_NOTIFY_EVENT_DATA,_:u32)->u32 {
    if !context.is_null(){(&*context.cast::<AtomicBool>()).store(true,Ordering::Release);}
    0
}
impl Watcher {
    pub fn open()->Result<Self> {
        let mut filter:CM_NOTIFY_FILTER=unsafe{std::mem::zeroed()};
        filter.cbSize=std::mem::size_of::<CM_NOTIFY_FILTER>() as u32;
        filter.Flags=CM_NOTIFY_FILTER_FLAG_ALL_DEVICE_INSTANCES;
        filter.FilterType=CM_NOTIFY_FILTER_TYPE_DEVICEINSTANCE;
        let dirty=Box::new(AtomicBool::new(false));let mut handle=std::ptr::null_mut();
        let status=unsafe{CM_Register_Notification(&filter,(&*dirty as *const AtomicBool).cast(),Some(callback),&mut handle)};
        ensure!(status==CR_SUCCESS,"native device notification registration failed: {status}");
        Ok(Self{handle,dirty})
    }
    pub fn take(&self)->bool {self.dirty.swap(false,Ordering::AcqRel)}
    fn close(&mut self)->Result<()> {
        if self.handle.is_null(){return Ok(())}
        let status=unsafe{CM_Unregister_Notification(self.handle)};
        ensure!(status==CR_SUCCESS,"native device notification cancellation failed: {status}");
        self.handle=std::ptr::null_mut();Ok(())
    }
}
impl Drop for Watcher {
    fn drop(&mut self){
        if let Err(error)=self.close() {
            // A failed unregister must never free memory still reachable by a native callback.
            let retained=std::mem::replace(&mut self.dirty,Box::new(AtomicBool::new(false)));
            let _=Box::leak(retained);
            eprintln!("device notification cancellation failed; callback context retained: {error}");
        }
    }
}
pub struct NetworkWatcher {handles:[HANDLE;3],dirty:Box<AtomicBool>}
// Same stable callback-context ownership as the PnP registration. Cancellation is outside callbacks.
unsafe impl Send for NetworkWatcher {}
unsafe fn network_changed(context:*const core::ffi::c_void,kind:MIB_NOTIFICATION_TYPE) {
    if !context.is_null()&&kind!=MibInitialNotification {(&*context.cast::<AtomicBool>()).store(true,Ordering::Release);}
}
unsafe extern "system" fn interface_callback(context:*const core::ffi::c_void,_:*const MIB_IPINTERFACE_ROW,kind:MIB_NOTIFICATION_TYPE){network_changed(context,kind);}
unsafe extern "system" fn address_callback(context:*const core::ffi::c_void,_:*const MIB_UNICASTIPADDRESS_ROW,kind:MIB_NOTIFICATION_TYPE){network_changed(context,kind);}
unsafe extern "system" fn route_callback(context:*const core::ffi::c_void,_:*const MIB_IPFORWARD_ROW2,kind:MIB_NOTIFICATION_TYPE){network_changed(context,kind);}
impl NetworkWatcher {
    pub fn open()->Result<Self> {
        let mut watcher=Self{handles:[std::ptr::null_mut();3],dirty:Box::new(AtomicBool::new(false))};
        let context=(&*watcher.dirty as *const AtomicBool).cast();
        let status=unsafe{NotifyIpInterfaceChange(AF_UNSPEC,Some(interface_callback),context,false,&mut watcher.handles[0])};
        ensure!(status==0,"native interface notification registration failed: {status}");
        let status=unsafe{NotifyUnicastIpAddressChange(AF_UNSPEC,Some(address_callback),context,false,&mut watcher.handles[1])};
        ensure!(status==0,"native address notification registration failed: {status}");
        let status=unsafe{NotifyRouteChange2(AF_UNSPEC,Some(route_callback),context,false,&mut watcher.handles[2])};
        ensure!(status==0,"native route notification registration failed: {status}");
        Ok(watcher)
    }
    pub fn take(&self)->bool {self.dirty.swap(false,Ordering::AcqRel)}
    fn close(&mut self)->Result<()> {
        let mut error=None;
        for handle in &mut self.handles {
            if handle.is_null(){continue}
            let status=unsafe{CancelMibChangeNotify2(*handle)};
            if status==0 {*handle=std::ptr::null_mut();}else{error=Some(status);}
        }
        ensure!(error.is_none(),"native network notification cancellation failed: {error:?}");Ok(())
    }
}
impl Drop for NetworkWatcher {
    fn drop(&mut self){
        if let Err(error)=self.close() {
            let retained=std::mem::replace(&mut self.dirty,Box::new(AtomicBool::new(false)));
            let _=Box::leak(retained);
            eprintln!("network notification cancellation failed; callback context retained: {error}");
        }
    }
}
#[derive(Default)]pub struct Coalescer {first:Option<Instant>,last:Option<Instant>}
impl Coalescer {
    pub fn poll(&mut self,dirty:bool,now:Instant)->bool {
        if dirty {self.first.get_or_insert(now);self.last=Some(now);}
        let ready=self.first.is_some_and(|first|now.duration_since(first)>=Duration::from_secs(10))||self.last.is_some_and(|last|now.duration_since(last)>=Duration::from_secs(2));
        if ready {self.first=None;self.last=None;}
        ready
    }
}
#[cfg(test)]mod tests {
    use super::*;
    #[test]fn burst_is_coalesced_and_continuous_storm_cannot_starve_reconciliation(){
        let now=Instant::now();let mut c=Coalescer::default();
        assert!(!c.poll(true,now));assert!(!c.poll(true,now+Duration::from_secs(1)));
        assert!(!c.poll(false,now+Duration::from_secs(2)));assert!(c.poll(false,now+Duration::from_secs(3)));
        assert!(!c.poll(false,now+Duration::from_secs(4)));
        for second in 5..15 {assert!(!c.poll(true,now+Duration::from_secs(second)));}
        assert!(c.poll(true,now+Duration::from_secs(15)));
    }
    #[test]fn native_registration_and_cancellation_are_readonly()->Result<()> {let mut watcher=Watcher::open()?;let _=watcher.take();watcher.close()?;assert!(watcher.handle.is_null());drop(watcher);Ok(())}
    #[test]fn native_network_registration_and_cancellation_are_readonly()->Result<()> {
        let mut watcher=NetworkWatcher::open()?;let _=watcher.take();watcher.close()?;
        assert!(watcher.handles.iter().all(|handle|handle.is_null()));Ok(())
    }
    #[test]fn network_callbacks_coalesce_without_borrowing_rows_and_ignore_initial_confirmation() {
        let dirty=Box::new(AtomicBool::new(false));let context=(&*dirty as *const AtomicBool).cast();
        unsafe{interface_callback(context,std::ptr::null(),MibInitialNotification);}
        assert!(!dirty.load(Ordering::Acquire));
        for _ in 0..10000 {unsafe{address_callback(context,std::ptr::null(),MibParameterNotification);route_callback(context,std::ptr::null(),MibParameterNotification);}}
        assert!(dirty.swap(false,Ordering::AcqRel));assert!(!dirty.swap(false,Ordering::AcqRel));
    }
}
