use anyhow::Result;
fn protected_text(text: &str) -> Result<serde_json::Value> {
    if pcscope_contracts::sensitive::sensitive_text(text) {
        pcscope_contracts::sensitive::protect(&serde_json::json!(text))
    } else { Ok(serde_json::json!(text)) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metadata_preserves_commands_but_protects_secrets() {
        assert_eq!(protected_text("app.exe /uninstall").unwrap(), "app.exe /uninstall");
        for text in ["app --password abc", "https://user:pass@example.com", "key --token abc"] {
            let protected = protected_text(text).unwrap();
            assert_eq!(protected["sensitive"], true);
            assert_eq!(pcscope_contracts::sensitive::unprotect(&protected).unwrap(),text);
            assert!(!protected.to_string().contains(text));
        }
    }
}
#[cfg(windows)]
fn main() -> Result<()> {
    use pcscope_contracts::{Envelope, Item, Message};
    use serde_json::json;
    use winreg::{enums::*, RegKey};
    let now = chrono::Utc::now().to_rfc3339();
    // Machine scope only: a service's HKCU must never be mistaken for an interactive user.
    let root = RegKey::predef(HKEY_LOCAL_MACHINE);
    let identity: String = root
        .open_subkey_with_flags(
            r"SOFTWARE\Microsoft\Cryptography",
            KEY_READ | KEY_WOW64_64KEY,
        )?
        .get_value("MachineGuid")?;
    for (view, flag) in [("64", KEY_WOW64_64KEY), ("32", KEY_WOW64_32KEY)] {
        let mut items = Vec::new();
        let mut complete = true;
        let path = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
        match root.open_subkey_with_flags(path, KEY_READ | flag) {
            Ok(uninstall) => {
                for key in uninstall.enum_keys() {
                    let key = match key {
                        Ok(v) => v,
                        Err(e) => {
                            complete = false;
                            eprintln!("registry enumeration failed: {e}");
                            continue;
                        }
                    };
                    let app = match uninstall.open_subkey_with_flags(&key, KEY_READ | flag) {
                        Ok(v) => v,
                        Err(e) => {
                            complete = false;
                            eprintln!("registry entry unavailable: {e}");
                            continue;
                        }
                    };
                    let mut data = serde_json::Map::new();
                    data.insert(
                        "registry_path".into(),
                        json!(format!("HKLM\\{path}\\{key}")),
                    );
                    data.insert("registry_view".into(), json!(view));
                    let mut values = serde_json::Map::new();
                    for entry in app.enum_values() {
                        match entry {
                            Ok((name, value)) => {
                                let text = value.to_string();
                                let lower = name.to_lowercase();
                                let sensitive = ["password", "passwd", "secret", "token", "credential", "privatekey", "productkey", "digitalproductid"]
                                    .iter().any(|word| lower.contains(word));
                                let projected = if sensitive {
                                    let original = match value.vtype {
                                        REG_SZ | REG_EXPAND_SZ => json!(text),
                                        REG_MULTI_SZ => json!(app.get_value::<Vec<String>, _>(&name)?),
                                        REG_DWORD => json!(app.get_value::<u32, _>(&name)?),
                                        REG_QWORD => json!(app.get_value::<u64, _>(&name)?),
                                        _ => json!({"encoding":"byte-array","bytes":value.bytes.len(),"value":value.bytes}),
                                    };
                                    pcscope_contracts::sensitive::protect(&original)?
                                } else {
                                    match value.vtype {
                                        REG_SZ | REG_EXPAND_SZ => protected_text(&text)?,
                                        REG_MULTI_SZ => match app.get_value::<Vec<String>, _>(&name) {
                                            Ok(parts) => json!(parts.iter().map(|s| protected_text(s)).collect::<Result<Vec<_>>>()?),
                                            Err(e) => { complete = false; json!({"unavailable":e.to_string()}) }
                                        },
                                        REG_DWORD => match app.get_value::<u32, _>(&name) {
                                            Ok(n) => json!(n), Err(e) => { complete = false; json!({"unavailable":e.to_string()}) }
                                        },
                                        REG_QWORD => match app.get_value::<u64, _>(&name) {
                                            Ok(n) => json!(n), Err(e) => { complete = false; json!({"unavailable":e.to_string()}) }
                                        },
                                        _ => json!({"encoding":"byte-array","bytes":value.bytes.len(),"value":value.bytes}),
                                    }
                                };
                                // Keep legacy display fields while retaining every other registration value.
                                if ["DisplayName","DisplayVersion","Publisher","InstallLocation","InstallDate"].contains(&name.as_str()) {
                                    data.insert(name.clone(), projected.clone());
                                }
                                values.insert(name, json!({"kind":format!("{:?}",value.vtype),"value":projected}));
                            }
                            Err(e) => { complete = false; eprintln!("registry values unavailable: {e}"); }
                        }
                    }
                    data.insert("values".into(), values.into());
                    data.insert("collection".into(), json!({"policy":"software-metadata-v2","scope":"HKLM 32/64-bit uninstall registration; not other user hives or portable software"}));
                    // Include unnamed registrations; absence of DisplayName is not absence of the object.
                    items.push(Item {
                        id: key.to_lowercase(),
                        data: data.into(),
                    });
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                complete = false;
                eprintln!("registry view unavailable: {e}");
            }
        }
        items.sort_by(|a, b| a.id.cmp(&b.id));
        if !complete {
            let gap = Envelope { protocol_version: 1, request_id: format!("software-{view}-{now}-gap"), message: Message::Gap {
                source: "pcscope.software".into(), observed_at: now.clone(),
                reason: format!("Software registry view {view} could not be read completely; previous state retained. See collector diagnostics."),
            }};
            println!("{}", serde_json::to_string(&gap)?);
            continue;
        }
        let env = Envelope {
            protocol_version: 1,
            request_id: format!("software-{view}-{now}"),
            message: Message::Snapshot {
                source: "pcscope.software".into(),
                scope: format!("machine:{identity}:hklm:{view}"),
                run_id: format!("{now}-{}", std::process::id()),
                observed_at: now.clone(),
                complete,
                items,
            },
        };
        let line = serde_json::to_string(&env)?;
        if line.len() + 1 > pcscope_contracts::MAX_LINE_BYTES {
            anyhow::bail!("inventory exceeds atomic snapshot limit; streaming required");
        }
        println!("{line}");
    }
    Ok(())
}

#[cfg(not(windows))]
fn main() -> Result<()> {
    anyhow::bail!("Windows required")
}
