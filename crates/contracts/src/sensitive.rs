use anyhow::{bail, Result};
use serde_json::{json, Value};

pub const MASK: &str = "******";
const SCHEME: &str = "dpapi-current-user-v1";

#[cfg(windows)]
fn crypt(bytes: &[u8], encrypt: bool) -> Result<Vec<u8>> {
    use windows_sys::Win32::{Foundation::LocalFree, Security::Cryptography::{CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN}};
    let mut input = CRYPT_INTEGER_BLOB { cbData: bytes.len().try_into()?, pbData: bytes.as_ptr() as *mut u8 };
    let entropy = b"PCScope/evidence/v1";
    let mut entropy = CRYPT_INTEGER_BLOB { cbData: entropy.len() as u32, pbData: entropy.as_ptr() as *mut u8 };
    let mut output = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
    unsafe {
        let ok = if encrypt { CryptProtectData(&mut input, std::ptr::null(), &mut entropy, std::ptr::null(), std::ptr::null_mut(), CRYPTPROTECT_UI_FORBIDDEN, &mut output) }
        else { CryptUnprotectData(&mut input, std::ptr::null_mut(), &mut entropy, std::ptr::null(), std::ptr::null_mut(), CRYPTPROTECT_UI_FORBIDDEN, &mut output) };
        if ok == 0 { return Err(std::io::Error::last_os_error().into()); }
        let value = if output.cbData == 0 { Vec::new() } else { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
        // Clear the native plaintext buffer before releasing it.
        if output.cbData > 0 { std::ptr::write_bytes(output.pbData, 0, output.cbData as usize); }
        LocalFree(output.pbData.cast());
        Ok(value)
    }
}
#[cfg(not(windows))]
fn crypt(_: &[u8], _: bool) -> Result<Vec<u8>> { bail!("DPAPI requires Windows") }

pub fn protect(value: &Value) -> Result<Value> {
    let mut plaintext = serde_json::to_vec(value)?;
    let encrypted = crypt(&plaintext, true);
    plaintext.fill(0);
    let ciphertext: String = encrypted?.iter().map(|byte| format!("{byte:02x}")).collect();
    Ok(json!({"sensitive":true,"protection":SCHEME,"ciphertext":ciphertext}))
}
pub fn unprotect(value: &Value) -> Result<Value> {
    if value["protection"] != SCHEME { bail!("Unknown sensitive value format"); }
    let hex = value["ciphertext"].as_str().ok_or_else(|| anyhow::anyhow!("Missing ciphertext"))?;
    if hex.len() % 2 != 0 || hex.len() > 2_097_152 || !hex.is_ascii() { bail!("Invalid ciphertext length"); }
    let bytes = (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i+2], 16)).collect::<std::result::Result<Vec<_>,_>>()?;
    let mut plaintext = crypt(&bytes, false)?;
    let result = serde_json::from_slice(&plaintext);
    plaintext.fill(0);
    Ok(result?)
}

pub fn sensitive_name(name: &str) -> bool {
    let name = name.to_lowercase().replace(['_', '-', ' '], "");
    if ["required","enabled","lastset","expires","age","length","history","policy","type","status"].iter().any(|suffix| name.ends_with(suffix)) { return false; }
    ["password","passwd","secret","credential","privatekey","recoverypassword","accesstoken","refreshtoken","apikey","authorization","digitalproductid","productkey"].iter().any(|word| name.contains(word)) || name == "token"
}
pub fn sensitive_text(text: &str) -> bool {
    let lower = text.to_lowercase();
    ["password", "passwd", "pwd=", "--token", "token=", "api_key", "api-key", "authorization", "private key", "secret=", "secret:", "bearer ", "-encodedcommand ", "-enc "].iter().any(|marker| lower.contains(marker)) || (text.contains("://") && text.contains('@'))
}

// Run at the IPC boundary: masked mode never returns ciphertext or plaintext to the UI.
pub fn present(value: &mut Value, reveal: bool) {
    if value.get("sensitive") == Some(&Value::Bool(true)) {
        *value = if reveal { unprotect(value).unwrap_or_else(|_| json!({"display":MASK,"unavailable":"无法使用当前 Windows 账户解密；加密原值仍保留"})) } else { json!(MASK) };
        return;
    }
    if value.get("redacted") == Some(&Value::Bool(true)) {
        *value = json!({"display":MASK,"unavailable":"旧版未保存原值，需等待来源重新采集"});
        return;
    }
    match value {
        Value::Array(items) => for item in items { present(item, reveal); },
        Value::Object(fields) => for (key, item) in fields {
            if !reveal && sensitive_name(key) && !item.is_null() && !item.is_boolean() { *item = json!(MASK); }
            else { present(item, reveal); }
        },
        Value::String(text) => {
            // SQLite can return an extracted JSON object as a string (e.g. a display name).
            if let Ok(mut embedded) = serde_json::from_str::<Value>(text) {
                if embedded.get("sensitive") == Some(&Value::Bool(true)) {
                    present(&mut embedded,reveal);*value=embedded;return;
                }
            }
            if !reveal && sensitive_text(text) { *value=json!(MASK); }
        },
        _ => {},
    }
}

// DPAPI uses random ciphertext. Compare decrypted values to avoid false history changes.
pub fn equivalent(left: &Value, right: &Value) -> bool {
    if left == right { return true; }
    if left.get("sensitive") == Some(&Value::Bool(true)) && right.get("sensitive") == Some(&Value::Bool(true)) {
        return match (unprotect(left), unprotect(right)) { (Ok(a),Ok(b)) => a == b, _ => false };
    }
    match (left,right) {
        (Value::Array(a),Value::Array(b)) => a.len()==b.len() && a.iter().zip(b).all(|(a,b)| equivalent(a,b)),
        (Value::Object(a),Value::Object(b)) => a.len()==b.len() && a.iter().all(|(k,v)| b.get(k).is_some_and(|other| equivalent(v,other))),
        _ => false,
    }
}

#[cfg(all(test,windows))]
mod tests {
    use super::*;
    #[test]
    fn protected_evidence_roundtrip_and_masking() {
        let secret=json!("fixture-secret-not-a-real-credential");
        let a=protect(&secret).unwrap();let b=protect(&secret).unwrap();
        assert!(!a.to_string().contains("fixture-secret"));assert!(equivalent(&a,&b));
        assert_eq!(unprotect(&a).unwrap(),secret);
        let mut masked=json!({"nested":[a.clone()]});present(&mut masked,false);assert_eq!(masked["nested"][0],MASK);
        let mut extracted=json!({"name":a.to_string()});present(&mut extracted,false);assert_eq!(extracted["name"],MASK);
        present(&mut a.clone(),true);
        let mut clear=a;present(&mut clear,true);assert_eq!(clear,secret);
    }
    #[test]
    fn legacy_and_corrupt_values_are_visible_not_silently_dropped() {
        let mut legacy=json!({"redacted":true});present(&mut legacy,false);assert_eq!(legacy["display"],MASK);
        let mut bad=json!({"sensitive":true,"protection":SCHEME,"ciphertext":"zz"});present(&mut bad,true);assert_eq!(bad["display"],MASK);assert!(bad["unavailable"].is_string());
    }
    #[test]
    fn powershell_dpapi_interoperability() {
        if let Ok(fixture)=std::env::var("PCSCOPE_DPAPI_TEST_FIXTURE") {
            assert_eq!(unprotect(&serde_json::from_str(&fixture).unwrap()).unwrap(),"pcscope-dpapi-fixture");
        }
    }
}
