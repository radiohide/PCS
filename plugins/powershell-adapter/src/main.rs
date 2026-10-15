use anyhow::{Context, Result};
use std::io::Read;
fn gap(source:&str,error:&str) {
    let now=chrono::Utc::now().to_rfc3339();
    println!("{}",serde_json::json!({"protocol_version":1,"request_id":format!("backend-failure/{source}/{now}"),"type":"gap","source":source,"observed_at":now,"reason":error}));
}
fn sources(unit:&str)->Vec<String> {
    let inventory:serde_json::Value=serde_json::from_str(include_str!("../../../model/sources.json")).expect("generated inventory");
    inventory["sources"].as_array().unwrap().iter().filter(|row|row["execution_units"].as_array().is_some_and(|units|units.iter().any(|id|id==unit))).filter_map(|row|row["source"].as_str().map(str::to_owned)).collect()
}
fn disabled(request:&serde_json::Value)->Vec<String> {request["selection"]["disabled_sources"].as_array().map(|rows|rows.iter().filter_map(|row|row.as_str().map(str::to_owned)).collect()).unwrap_or_default()}
fn main() -> Result<()> {
    let began=std::time::Instant::now();
    let domain = std::env::args().nth(1);
    if domain.as_deref().is_some_and(|d| {
        ![
            "system",
            "security",
            "network",
            "storage",
            "startup",
            "desktop",
            "virtualization",
            "applications",
            "scoped",
        ]
        .contains(&d)
    }) {
        anyhow::bail!("unsupported collection domain");
    }
    let script = std::env::current_exe()?.with_file_name(if domain.is_some() {
        "collect-domain.ps1"
    } else {
        "collect-system.ps1"
    });
    let script_exists=script.is_file();
    let windows = std::env::var_os("SystemRoot").context("SystemRoot missing")?;
    let powershell =
        std::path::PathBuf::from(windows).join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let mut args: Vec<std::ffi::OsString> = vec![
        "-NoLogo".into(),
        "-NoProfile".into(),
        "-NonInteractive".into(),
        "-ExecutionPolicy".into(),
        "RemoteSigned".into(),
        "-File".into(),
        script.into_os_string(),
    ];
    let mut bytes=Vec::new();std::io::stdin().take(65537).read_to_end(&mut bytes)?;
    anyhow::ensure!(bytes.len()<=65536,"collector request exceeds limit");
    let mut request:serde_json::Value=if bytes.is_empty(){serde_json::json!({})}else{serde_json::from_slice(&bytes)?};
    let unit=domain.as_deref().unwrap_or("pcscope-powershell-adapter.exe");let all=sources(unit);
    let ownership:serde_json::Value=serde_json::from_str(include_str!("../../platform-native/ownership.json"))?;
    let owned:Vec<String>=ownership[unit].as_array().map(|rows|rows.iter().filter_map(|row|row.as_str().map(str::to_owned)).collect()).unwrap_or_default();
    let original_disabled=disabled(&request);let mut native_enabled:Vec<_>=owned.iter().filter(|source|!original_disabled.contains(source)).cloned().collect();
    if unit=="scoped" {
        let wants_optional=native_enabled.iter().any(|source|matches!(source.as_str(),"pcscope.apps.mihomo"|"pcscope.software.portable"|"pcscope.virtualization.docker"));
        if wants_optional {
            let options:serde_json::Value=serde_json::from_slice(&std::fs::read(std::env::current_exe()?.with_file_name("collector-options.json"))?)?;
            native_enabled.retain(|source|match source.as_str(){
                "pcscope.apps.mihomo"=>options["mihomo_metadata"]==true,
                "pcscope.virtualization.docker"=>options["docker_metadata"]==true,
                "pcscope.software.portable"=>options["portable_roots"].as_array().is_some_and(|rows|!rows.is_empty()),
                _=>true
            });
        }
        native_enabled.retain(|source|match source.as_str(){
            "pcscope.files.metadata-security"=>request["files"].as_array().is_some_and(|rows|!rows.is_empty()),
            "pcscope.files.directory-security"=>request["directories"].as_array().is_some_and(|rows|!rows.is_empty()),
            _=>true
        });
    }
    if !native_enabled.is_empty() {
        let executable=std::env::current_exe()?.with_file_name("pcscope-platform-native.exe");
        let mut native_request=request.clone();if !native_request["selection"].is_object(){native_request["selection"]=serde_json::json!({});}
        let mut excludes=original_disabled.clone();excludes.extend(all.iter().filter(|source|!native_enabled.contains(source)).cloned());native_request["selection"]["disabled_sources"]=serde_json::json!(excludes);
        match pcscope_supervisor::collect_with_input(&executable,&[unit.into()],Some(serde_json::to_vec(&native_request)?),std::time::Duration::from_secs(55)) {
            Ok(result)=>for message in result.messages{println!("{}",serde_json::to_string(&message)?);},
            Err(error)=>for source in &native_enabled{gap(source,&format!("Native collector failed; source state retained; no PowerShell fallback: {error}"));},
        }
    }
    let remaining:Vec<_>=all.iter().filter(|source|!owned.contains(source)&&!original_disabled.contains(source)).cloned().collect();
    if remaining.is_empty(){return Ok(());}
    if !script_exists{for source in &remaining{gap(source,"PowerShell compatibility script missing; native evidence remains valid; source state retained");}return Ok(());}
    if unit=="scoped" {
        let options:serde_json::Value=serde_json::from_slice(&std::fs::read(std::env::current_exe()?.with_file_name("collector-options.json"))?)?;
        let configured=options["docker_metadata"]==true||options["mihomo_metadata"]==true||options["portable_roots"].as_array().is_some_and(|a|!a.is_empty());
        if !configured{return Ok(());}
    }
    if !request["selection"].is_object(){request["selection"]=serde_json::json!({});}
    let mut excluded=original_disabled;excluded.extend(owned);request["selection"]["disabled_sources"]=serde_json::json!(excluded);
    if request["selection"].is_object(){args.push("-SelectionJson".into());args.push(serde_json::to_string(&request["selection"])?.into());}
    if domain.as_deref()==Some("scoped") {
        let files=request["files"].as_array().cloned().unwrap_or_default();
        args.push("-WatchListJson".into());args.push(serde_json::to_string(&files)?.into());
        args.push("-DirectoryListJson".into());args.push(serde_json::to_string(&request["directories"].as_array().cloned().unwrap_or_default())?.into());
    }
    if let Some(domain) = domain {
        args.push("-Domain".into());
        args.push(domain.into());
    }
    let result = match pcscope_supervisor::collect_with_args(
        &powershell,
        &args,
        std::time::Duration::from_secs(110).saturating_sub(began.elapsed()).min(std::time::Duration::from_secs(60)),
    ) {Ok(result)=>result,Err(error)=>{for source in &remaining{gap(source,&format!("PowerShell compatibility collector unavailable; source state retained: {error}"));}return Ok(());}};
    for message in result.messages {
        println!("{}", serde_json::to_string(&message)?);
    }
    if !result.diagnostic.is_empty() {
        eprintln!("PowerShell diagnostic bytes: {}", result.diagnostic.len());
    }
    Ok(())
}

#[cfg(test)]mod tests {
    use super::*;
    #[test]fn native_ownership_has_no_unknown_or_duplicated_source(){let mapping:serde_json::Value=serde_json::from_str(include_str!("../../platform-native/ownership.json")).unwrap();let mut seen=std::collections::BTreeSet::new();for unit in ["pcscope-powershell-adapter.exe","system","security","storage","network","startup","desktop","virtualization","applications","scoped"]{let actual=sources(unit);for source in mapping[unit].as_array().unwrap(){let source=source.as_str().unwrap();assert!(actual.iter().any(|s|s==source),"{unit}/{source}");assert!(seen.insert(source));}}}
}
