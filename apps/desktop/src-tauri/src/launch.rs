use std::{ffi::OsString, path::PathBuf};
use std::io::Read;

#[derive(Default, Debug, PartialEq)]
pub struct Options {
    pub data_dir: Option<PathBuf>,
    pub webview_dir: Option<PathBuf>,
    pub runtime_dir: Option<PathBuf>,
}

/// Shortcuts select the same storage explicitly, without a shell or inherited developer env.
pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Options, String> {
    let mut args = args.into_iter();
    let mut options = Options::default();
    while let Some(argument) = args.next() {
        let slot = match argument.to_str() {
            Some("--data-dir") => &mut options.data_dir,
            Some("--webview-dir") => &mut options.webview_dir,
            Some("--runtime-dir") => &mut options.runtime_dir,
            _ => return Err("启动参数无法识别，请重新创建程序快捷方式。".into()),
        };
        if slot.is_some() { return Err("启动目录重复指定，请重新创建程序快捷方式。".into()); }
        let path = PathBuf::from(args.next().ok_or("启动目录未指定。")?);
        if !path.is_absolute() { return Err("启动目录必须是完整路径。".into()); }
        *slot = Some(path);
    }
    Ok(options)
}

pub static OVERRIDES: std::sync::OnceLock<[bool;2]> = std::sync::OnceLock::new();

pub fn configure() -> Result<(), String> {
    let mut options = parse(std::env::args_os().skip(1))?;
    let directory=std::env::current_exe().map_err(|e|e.to_string())?
        .parent().ok_or("无法确定程序目录。")?.to_owned();
    let _=OVERRIDES.set([options.data_dir.is_some()||std::env::var_os("PCSCOPE_DATA_DIR").is_some(),options.runtime_dir.is_some()||std::env::var_os("PCSCOPE_RUNTIME_DIR").is_some()]);
    let defaults=read_defaults(&directory.join("pcscope-launch.json"))?;
    // Explicit arguments and environment retain priority; a sidecar makes a complete debug
    // bundle directly runnable without depending on the working directory or a shortcut.
    if options.data_dir.is_none() && std::env::var_os("PCSCOPE_DATA_DIR").is_none() {
        options.data_dir=defaults.data_dir;
    }
    if options.webview_dir.is_none() && std::env::var_os("WEBVIEW2_USER_DATA_FOLDER").is_none() {
        options.webview_dir=defaults.webview_dir;
    }
    if options.runtime_dir.is_none() && std::env::var_os("PCSCOPE_RUNTIME_DIR").is_none() { options.runtime_dir=defaults.runtime_dir; }
    if let Some(path) = options.runtime_dir { std::env::set_var("PCSCOPE_RUNTIME_DIR", path); }
    if let Some(path) = options.data_dir { std::env::set_var("PCSCOPE_DATA_DIR", path); }
    if let Some(path) = options.webview_dir { std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", path); }
    Ok(())
}

pub fn read_defaults(path: &std::path::Path) -> Result<Options,String> {
    let file=match std::fs::File::open(path) {
        Ok(file)=>file,
        Err(error) if error.kind()==std::io::ErrorKind::NotFound=>return Ok(Options::default()),
        Err(error)=>return Err(format!("无法读取启动配置：{error}")),
    };
    let mut bytes=Vec::new();
    file.take(16385).read_to_end(&mut bytes).map_err(|e|format!("无法读取启动配置：{e}"))?;
    if bytes.len()>16384 {return Err("启动配置过大，请重新生成。".into());}
    decode_defaults(&bytes)
}

fn decode_defaults(bytes: &[u8]) -> Result<Options,String> {
    let value:serde_json::Value=serde_json::from_slice(bytes.strip_prefix(&[0xef,0xbb,0xbf]).unwrap_or(bytes))
        .map_err(|e|format!("启动配置格式错误：{e}"))?;
    let object=value.as_object().ok_or("启动配置格式错误。")?;
    if value["version"]!=1 || object.keys().any(|key|!matches!(key.as_str(),"version"|"data_dir"|"webview_dir"|"runtime_dir")) {
        return Err("启动配置版本或字段不受支持。".into());
    }
    let path=|key:&str|->Result<PathBuf,String>{
        let path=PathBuf::from(value[key].as_str().ok_or_else(||format!("启动配置缺少 {key}。"))?);
        if !path.is_absolute(){return Err("启动配置必须使用完整目录路径。".into());}
        Ok(path)
    };
    Ok(Options{data_dir:Some(path("data_dir")?),webview_dir:object.contains_key("webview_dir").then(||path("webview_dir")).transpose()?,runtime_dir:object.contains_key("runtime_dir").then(||path("runtime_dir")).transpose()?})
}

pub fn runtime_dir() -> Result<PathBuf,String> {
    let path=match std::env::var_os("PCSCOPE_RUNTIME_DIR") {
        Some(value)=>PathBuf::from(value),
        None=>std::env::current_exe().map_err(|e|e.to_string())?.parent().ok_or("无法确定程序目录。")?.to_owned(),
    };
    if !path.is_absolute(){return Err("后台及采集程序目录必须是完整路径。".into());}
    Ok(path)
}

pub fn service_executable() -> Result<PathBuf,String> { Ok(runtime_dir()?.join("pcscope-service.exe")) }

fn failure_summary(detail: &str) -> &'static str {
    if detail.contains("拒绝访问") || detail.contains("os error 5") || detail.contains("Access is denied") {
        "程序无法访问启动所需的文件，窗口未能打开。"
    } else { "PCScope 未能打开。" }
}

/// The Windows subsystem has no terminal. A startup failure must not silently disappear.
pub fn report_failure(detail: &str) {
    eprintln!("PCScope startup failed: {detail}");
    let directory = std::env::current_exe().ok().and_then(|path| path.parent().map(ToOwned::to_owned));
    let logged = directory.as_ref().is_some_and(|path| {
        std::fs::write(path.join("pcscope-startup-error.log"), detail).is_ok()
    });
    let entry_exists = directory.as_ref().is_some_and(|path| path.join("打开 PCScope.lnk").is_file());
    let mut message = format!("{}\n\n{}", failure_summary(detail), if entry_exists {
        "请双击同目录的“打开 PCScope”入口。"
    } else { "请从完整程序目录重新打开 PCScope。" });
    if logged { message.push_str("\n\n详细原因已保存到同目录的 pcscope-startup-error.log。"); }
    #[cfg(windows)] unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
        let text: Vec<u16> = message.encode_utf16().chain(Some(0)).collect();
        let title: Vec<u16> = "PCScope 无法启动".encode_utf16().chain(Some(0)).collect();
        MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), MB_OK | MB_ICONERROR);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_shortcut_arguments_preserves_default_launch() {
        assert_eq!(parse(Vec::<OsString>::new()).unwrap(), Options::default());
    }
    #[test]
    fn rejects_incomplete_relative_unknown_or_duplicate_arguments() {
        for args in [vec!["--data-dir"], vec!["--data-dir", "relative"], vec!["--command", "cmd"]] {
            assert!(parse(args.into_iter().map(OsString::from)).is_err());
        }
        let path=std::env::temp_dir();
        assert!(parse(["--data-dir".into(),path.clone().into_os_string(),"--data-dir".into(),path.into_os_string()]).is_err());
    }
    #[test]
    fn shortcut_selects_exact_paths_even_with_spaces() {
        let data=std::env::temp_dir().join("PCScope test data");
        let cache=std::env::temp_dir().join("PCScope webview");
        let options=parse(["--data-dir".into(),data.clone().into_os_string(),"--webview-dir".into(),cache.clone().into_os_string()]).unwrap();
        assert_eq!(options,Options {data_dir:Some(data),webview_dir:Some(cache),runtime_dir:None});
    }
    #[test]
    fn access_failure_has_readable_impact() {
        assert!(failure_summary("setup hook: 拒绝访问。 (os error 5)").contains("无法访问"));
        assert_eq!(failure_summary("other startup error"),"PCScope 未能打开。");
    }
    #[test]
    fn sidecar_preserves_the_exact_selected_directories() {
        let root=std::env::temp_dir();
        let data=root.join("PCScope existing records");let cache=root.join("PCScope cache");
        let bytes=serde_json::to_vec(&serde_json::json!({"version":1,"data_dir":data,"webview_dir":cache})).unwrap();
        let expected=Options{data_dir:Some(data),webview_dir:Some(cache),runtime_dir:None};
        assert_eq!(decode_defaults(&bytes).unwrap(),expected);
        assert_eq!(decode_defaults(&[&[0xef,0xbb,0xbf][..],&bytes].concat()).unwrap(),expected);
    }
    #[test]
    fn invalid_sidecar_cannot_silently_select_another_database() {
        for bytes in [br#"{}"#.as_slice(),br#"{"version":2}"#,br#"{"version":1,"data_dir":"relative","webview_dir":"relative"}"#,br#"{"version":1,"command":"anything"}"#] {
            assert!(decode_defaults(bytes).is_err());
        }
        assert_eq!(read_defaults(&std::env::temp_dir().join(format!("pcscope-missing-launch-{}.json",std::process::id()))).unwrap(),Options::default());
    }
}
