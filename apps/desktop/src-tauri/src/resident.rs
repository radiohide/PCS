//! One resident desktop per account/data-directory endpoint. No database or collector access.
use tauri::{Manager, menu::{Menu, MenuItem, PredefinedMenuItem}, tray::{TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState}};
use std::sync::Arc;

pub fn show(app:&tauri::AppHandle) {
    if let Some(window)=app.get_webview_window("main") {
        let _=window.unminimize();let _=window.show();let _=window.set_focus();
    }
}

pub struct Status { item:MenuItem<tauri::Wry>, tray:tauri::tray::TrayIcon<tauri::Wry>, failures:u8, connected:Option<bool> }
impl Status {
    pub fn update(&mut self,ok:bool) {
        self.failures=if ok{0}else{self.failures.saturating_add(1)};
        // A brief permission handover or busy reply should not flash an alarm in the tray.
        let next=if ok{Some(true)}else if self.failures>=3{Some(false)}else{self.connected};
        if next==self.connected{return}self.connected=next;
        let text=if next==Some(true){"后台已连接"}else{"后台暂未连接"};
        let _=self.item.set_text(text);let _=self.tray.set_tooltip(Some(format!("PCScope · {text}")));
    }
}

pub fn install(app:&tauri::App)->tauri::Result<Status> {
    let open=MenuItem::with_id(app,"resident-open","打开 PCScope",true,None::<&str>)?;
    let status=MenuItem::with_id(app,"resident-status","正在连接后台…",false,None::<&str>)?;
    let quit=MenuItem::with_id(app,"resident-quit","退出界面（后台继续采集）",true,None::<&str>)?;
    let separator=PredefinedMenuItem::separator(app)?;
    let menu=Menu::with_items(app,&[&open,&status,&separator,&quit])?;
    let icon=app.default_window_icon().ok_or_else(||std::io::Error::other("程序图标缺失"))?.clone();
    let tray=TrayIconBuilder::with_id("pcscope-resident").icon(icon).tooltip("PCScope · 正在连接后台…")
        .menu(&menu).show_menu_on_left_click(false)
        .on_menu_event(|app,event|match event.id.as_ref(){"resident-open"=>show(app),"resident-quit"=>app.exit(0),_=>{}})
        .on_tray_icon_event(|tray,event|{if matches!(event,TrayIconEvent::Click{button:MouseButton::Left,button_state:MouseButtonState::Up,..}){show(tray.app_handle());}})
        .build(app)?;
    Ok(Status{item:status,tray,failures:0,connected:None})
}

#[cfg(windows)]
pub struct Instance(usize);
#[cfg(windows)]
impl Drop for Instance {fn drop(&mut self){unsafe{windows_sys::Win32::Foundation::CloseHandle(self.0 as _);}}}
#[cfg(windows)]
impl Instance {
    /// Auto-reset event persists a wakeup even while the first process is still starting.
    /// The endpoint includes the account and canonical directory identity, so aliases converge
    /// while explicitly different data directories remain independent.
    pub fn acquire(instance:&str)->std::io::Result<Option<Arc<Self>>> {
        use windows_sys::Win32::{Foundation::{GetLastError,ERROR_ALREADY_EXISTS,CloseHandle},System::Threading::{CreateEventW,SetEvent}};
        let name:Vec<u16>=format!("Local\\PCScope.desktop.{instance}").encode_utf16().chain(Some(0)).collect();
        let handle=unsafe{CreateEventW(std::ptr::null(),0,0,name.as_ptr())};
        if handle.is_null(){return Err(std::io::Error::last_os_error())}
        if unsafe{GetLastError()}==ERROR_ALREADY_EXISTS {
            let signaled=unsafe{SetEvent(handle)};let error=std::io::Error::last_os_error();unsafe{CloseHandle(handle);}
            if signaled==0{return Err(error)}return Ok(None)
        }
        Ok(Some(Arc::new(Self(handle as usize))))
    }
    pub fn listen(self:Arc<Self>,app:tauri::AppHandle) {
        std::thread::spawn(move ||{use windows_sys::Win32::System::Threading::{WaitForSingleObject,INFINITE};
            while unsafe{WaitForSingleObject(self.0 as _,INFINITE)}==0 {
                let handle=app.clone();if app.run_on_main_thread(move ||show(&handle)).is_err(){break}
            }
        });
    }
}

#[cfg(all(test,windows))]
mod tests {
    use super::*;
    #[test] fn duplicate_launch_signals_existing_instance_and_release_allows_restart(){
        let name=format!("test-{}-{:?}",std::process::id(),std::time::SystemTime::now());
        let first=Instance::acquire(&name).unwrap().unwrap();
        assert!(Instance::acquire(&name).unwrap().is_none());
        assert_eq!(unsafe{windows_sys::Win32::System::Threading::WaitForSingleObject(first.0 as _,0)},0);
        assert!(Instance::acquire(&(name.clone()+"-other")).unwrap().is_some());
        drop(first);assert!(Instance::acquire(&name).unwrap().is_some());
    }
}
