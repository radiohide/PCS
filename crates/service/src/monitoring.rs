use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::{process::{Command, Stdio}, time::{Duration, Instant}, io::Read};

pub fn check(repair: bool) -> Result<Value> {
    let executable=std::env::current_exe()?.with_file_name("pcscope-platform-native.exe");
    anyhow::ensure!(executable.is_file(),"缺少原生能力检测程序；不会自动回退并产生 PowerShell 审计流量");
    let mut cmd = Command::new(executable);
    cmd.arg("capabilities");if repair {cmd.arg("--repair");}
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)] { use std::os::windows::process::CommandExt; cmd.creation_flags(0x08000000); }
    let mut child = cmd.spawn()?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let out = std::thread::spawn(move || {let mut bytes=Vec::new(); stdout.take(2_000_000).read_to_end(&mut bytes).map(|_| bytes)});
    let err = std::thread::spawn(move || {let mut bytes=Vec::new(); stderr.take(100_000).read_to_end(&mut bytes).map(|_| bytes)});
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {break status;}
        if started.elapsed() > Duration::from_secs(90) {let _=child.kill(); let _=child.wait(); bail!("采集能力检查超时；结果未知，请重新检测");}
        std::thread::sleep(Duration::from_millis(100));
    };
    let output = out.join().map_err(|_| anyhow::anyhow!("check output failed"))??;
    let error = err.join().map_err(|_| anyhow::anyhow!("check error failed"))??;
    if !status.success() {bail!("{}",String::from_utf8_lossy(&error));}
    Ok(serde_json::from_slice(&output)?)
}

pub fn settings(mode: &str, repair: bool) -> Result<Value> {
    if !["low", "enhanced"].contains(&mode) {bail!("未知运行模式");}
    if mode == "low" && repair {bail!("低权限模式不能自动配置审计");}
    Ok(json!({"mode":mode,"repair":repair,"configured":true}))
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn validate_modes() {
        assert!(settings("other",false).is_err());
        assert!(settings("low",true).is_err());
        assert_eq!(settings("enhanced",true).unwrap()["repair"],true);
        assert_eq!(settings("low",false).unwrap()["configured"],true);
    }
}
