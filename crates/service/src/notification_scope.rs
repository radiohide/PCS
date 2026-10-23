//! Notification requests are bounded native inventories, never a compatibility-script run.
use anyhow::{ensure,Result};
use serde_json::{json,Value};

pub const DEVICE_CORE:&[&str]=&["pcscope.hardware.devices","pcscope.storage.volumes","pcscope.network.adapters","pcscope.network.addresses"];
pub const DEVICE_STORAGE:&[&str]=&["pcscope.hardware.driver-packages","pcscope.hardware.driver-services","pcscope.hardware.driver-bindings","pcscope.storage.disks","pcscope.storage.partitions"];
pub const NETWORK_CORE:&[&str]=&["pcscope.network.adapters","pcscope.network.addresses"];
pub const NETWORK_ROUTES:&[&str]=&["pcscope.network.routes"];

pub fn restrict(selection:&mut Value,unit:&str,allowed:&[String])->Result<()> {
    let ownership:Value=serde_json::from_str(include_str!("../../../plugins/platform-native/ownership.json"))?;
    let native=ownership[unit].as_array().ok_or_else(||anyhow::anyhow!("notification unit has no native owner"))?;
    ensure!(!allowed.is_empty()&&allowed.iter().all(|s|native.contains(&json!(s))),"notification source must have native ownership");
    let inventory:Value=serde_json::from_str(include_str!("../../../model/sources.json"))?;
    let mut disabled=selection["disabled_sources"].as_array().cloned().unwrap_or_default();
    for row in inventory["sources"].as_array().unwrap() {
        if row["execution_units"].as_array().is_some_and(|units|units.contains(&json!(unit)))
            && !allowed.iter().any(|s|row["source"]==*s)&&!disabled.contains(&row["source"]) {
            disabled.push(row["source"].clone());
        }
    }
    selection["all_disabled"]=json!(selection["all_disabled"]==true||allowed.iter().all(|s|disabled.contains(&json!(s))));
    selection["disabled_sources"]=json!(disabled);
    selection["allowed_sources"]=json!(allowed);
    Ok(())
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn notifications_exclude_unrelated_inventory_and_preserve_user_disables()->Result<()> {
        for (unit,sources,excluded) in [
            ("pcscope-powershell-adapter.exe",NETWORK_CORE,"pcscope.shell.registrations"),
            ("pcscope-powershell-adapter.exe",DEVICE_CORE,"pcscope.startup.services"),
            ("network",NETWORK_ROUTES,"pcscope.network.firewall-rules"),
            ("storage",DEVICE_STORAGE,"pcscope.hardware.firmware")
        ] {
            let allowed=sources.iter().map(|s|s.to_string()).collect::<Vec<_>>();
            let mut selection=json!({"all_disabled":false,"disabled_sources":[sources[0]],"disabled_items":["keep"]});
            restrict(&mut selection,unit,&allowed)?;
            let disabled=selection["disabled_sources"].as_array().unwrap();
            assert!(disabled.contains(&json!(sources[0])));assert!(disabled.contains(&json!(excluded)));
            assert_eq!(selection["disabled_items"],json!(["keep"]));
            if sources.len()==1 {assert_eq!(selection["all_disabled"],true);}
        }
        assert!(restrict(&mut json!({}),"network",&["pcscope.shell.registrations".into()]).is_err());
        Ok(())
    }
}
