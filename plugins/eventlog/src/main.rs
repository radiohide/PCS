use anyhow::Result;
use std::io::Read;
fn main() -> Result<()> {
    let mut input = Vec::new();
    std::io::stdin().take(65537).read_to_end(&mut input)?;
    if input.len() > 65536 {
        anyhow::bail!("event log request exceeds limit");
    }
    let _: serde_json::Value = serde_json::from_slice(&input)?;
    // All event sources are native-owned. An old backend=powershell override no longer
    // starts a script host. In particular reading PowerShell logs cannot feed itself.
    let executable = std::env::current_exe()?.with_file_name("pcscope-eventlog-native.exe");
    let result = pcscope_supervisor::collect_with_input(
        &executable,
        &[],
        Some(input),
        std::time::Duration::from_secs(30),
    )?;
    for message in result.messages {
        println!("{}", serde_json::to_string(&message)?);
    }
    Ok(())
}
