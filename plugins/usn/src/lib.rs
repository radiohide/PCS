use anyhow::{bail, ensure, Result};
use serde_json::{json, Value};

#[derive(Debug)]
pub struct Record {
    pub usn: i64,
    pub timestamp: i64,
    pub file_id: String,
    pub parent_id: String,
    pub name: String,
    pub reason: u32,
    pub attributes: u32,
    pub version: u16,
}
fn u16_at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes(b[o..o + 2].try_into().unwrap())
}
fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn i64_at(b: &[u8], o: usize) -> i64 {
    i64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}
fn hex(b: &[u8]) -> String {
    b.iter().rev().map(|b| format!("{b:02x}")).collect()
}
pub fn parse_buffer(bytes: &[u8]) -> Result<(i64, Vec<Record>)> {
    ensure!(bytes.len() >= 8, "missing USN continuation header");
    let next = i64_at(bytes, 0);
    ensure!(next >= 0, "negative continuation USN");
    let mut records = Vec::new();
    let mut offset = 8;
    let mut previous = None;
    while offset < bytes.len() {
        let tail = &bytes[offset..];
        ensure!(tail.len() >= 8, "truncated record header");
        let length = u32_at(tail, 0) as usize;
        ensure!(
            length >= 8 && length <= tail.len() && length.is_multiple_of(8),
            "invalid record length"
        );
        let b = &tail[..length];
        let version = u16_at(b, 4);
        let (fixed, id_len, usn_offset) = match version {
            2 => (60, 8, 24),
            3 => (76, 16, 40),
            _ => bail!("unsupported USN record version {version}; checkpoint not advanced"),
        };
        ensure!(b.len() >= fixed, "truncated fixed record");
        let name_len = u16_at(b, fixed - 4) as usize;
        let name_offset = u16_at(b, fixed - 2) as usize;
        ensure!(
            name_len.is_multiple_of(2)
                && name_offset >= fixed
                && name_offset.is_multiple_of(2)
                && name_offset
                    .checked_add(name_len)
                    .is_some_and(|end| end <= length),
            "invalid UTF16 name range"
        );
        let utf16: Vec<u16> = b[name_offset..name_offset + name_len]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| u16::from_le_bytes([p[0], p[1]]))
            .collect();
        let name = String::from_utf16(&utf16)?;
        let usn = i64_at(b, usn_offset);
        ensure!(
            usn >= 0 && usn < next && previous.is_none_or(|p| usn > p),
            "USN sequence is not strictly increasing"
        );
        previous = Some(usn);
        records.push(Record {
            usn,
            timestamp: i64_at(b, usn_offset + 8),
            file_id: hex(&b[8..8 + id_len]),
            parent_id: hex(&b[8 + id_len..8 + id_len * 2]),
            name,
            reason: u32_at(b, usn_offset + 16),
            attributes: u32_at(b, usn_offset + 28),
            version,
        });
        offset += length;
    }
    Ok((next, records))
}

pub struct Position {
    pub epoch: String,
    pub sequence: u64,
    pub start: i64,
    pub gap: Option<String>,
}
pub fn resume_position(
    volume: &str,
    journal: &str,
    first: i64,
    next: i64,
    resume: Option<&Value>,
    new_epoch: &str,
) -> Result<Position> {
    ensure!(first >= 0 && next >= first, "invalid journal range");
    let mut position = Position {
        epoch: new_epoch.into(),
        sequence: 1,
        start: first,
        gap: None,
    };
    if let Some(resume) = resume {
        let cp: Value = serde_json::from_str(
            resume["checkpoint"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("checkpoint string missing"))?,
        )?;
        let old = cp["next_usn"]
            .as_i64()
            .ok_or_else(|| anyhow::anyhow!("next_usn missing"))?;
        if cp["volume"] != volume || cp["journal"] != journal || old > next {
            position.gap=Some("Volume/journal identity changed or cursor is ahead of source; a new epoch was started".into());
        } else {
            position.epoch = resume["epoch"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("epoch missing"))?
                .into();
            position.sequence = resume["batch_seq"]
                .as_u64()
                .and_then(|v| v.checked_add(1))
                .ok_or_else(|| anyhow::anyhow!("invalid sequence"))?;
            position.start = old.max(first);
            if old < first {
                position.gap = Some(format!(
                    "USN records [{old}, {first}) are no longer available; loss count unknown"
                ));
            }
        }
    }
    Ok(position)
}

pub fn checkpoint(volume: &str, journal: &str, next: i64, source_next: i64) -> String {
    json!({"volume":volume,"journal":journal,"next_usn":next,"source_next_usn":source_next,"has_more":next<source_next}).to_string()
}
pub fn matches_watch(parent: &str, name: &str, id64: &str, id128: &str, expected: &str) -> bool {
    (parent==id64 || parent==id128) && name.to_lowercase()==expected.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn watch_matches_exact_parent_and_filename() {
        assert!(matches_watch("abc","File.TXT","abc","000abc","file.txt"));
        assert!(matches_watch("000abc","file.txt","abc","000abc","file.txt"));
        assert!(!matches_watch("other","file.txt","abc","000abc","file.txt"));
        assert!(!matches_watch("abc","file.txt.bak","abc","000abc","file.txt"));
        assert!(!matches_watch("abc","file.txt","abc","000abc","*.txt"));
    }
    fn fixture(version: u16) -> Vec<u8> {
        let fixed = if version == 3 { 76 } else { 60 };
        let offset = if version == 3 { 40 } else { 24 };
        let name: Vec<u16> = "测试.txt".encode_utf16().collect();
        let len = (fixed + name.len() * 2).div_ceil(8) * 8;
        let mut b = vec![0u8; 8 + len];
        b[..8].copy_from_slice(&200i64.to_le_bytes());
        b[8..12].copy_from_slice(&(len as u32).to_le_bytes());
        b[12..14].copy_from_slice(&version.to_le_bytes());
        b[8 + offset..16 + offset].copy_from_slice(&100i64.to_le_bytes());
        b[8 + fixed - 4..8 + fixed - 2].copy_from_slice(&((name.len() * 2) as u16).to_le_bytes());
        b[8 + fixed - 2..8 + fixed].copy_from_slice(&(fixed as u16).to_le_bytes());
        for (i, v) in name.iter().enumerate() {
            b[8 + fixed + i * 2..10 + fixed + i * 2].copy_from_slice(&v.to_le_bytes())
        }
        b
    }
    #[test]
    fn parses_v2_and_v3_names_and_ids() {
        for v in [2, 3] {
            let (next, rows) = parse_buffer(&fixture(v)).unwrap();
            assert_eq!(next, 200);
            assert_eq!(rows[0].name, "测试.txt");
            assert_eq!(rows[0].file_id.len(), if v == 2 { 16 } else { 32 });
        }
    }
    #[test]
    fn rejects_truncation_invalid_lengths_versions_and_cursor() {
        let base = fixture(2);
        for n in 0..base.len() {
            if n != 8 {
                assert!(parse_buffer(&base[..n]).is_err());
            }
        }
        let mut b = base.clone();
        b[12..14].copy_from_slice(&4u16.to_le_bytes());
        assert!(parse_buffer(&b).is_err());
        let mut b = base.clone();
        b[66..68].copy_from_slice(&2u16.to_le_bytes());
        assert!(parse_buffer(&b).is_err());
        let mut b = base;
        b[..8].copy_from_slice(&50i64.to_le_bytes());
        assert!(parse_buffer(&b).is_err());
    }
    #[test]
    fn resume_detects_overwrite_identity_change_and_rollback() {
        let saved = json!({"epoch":"old","batch_seq":7,"checkpoint":checkpoint("v","j",100,100)});
        let p = resume_position("v", "j", 0, 200, Some(&saved), "new").unwrap();
        assert_eq!((p.start, p.sequence), (100, 8));
        assert!(p.gap.is_none());
        let p = resume_position("v", "j", 150, 200, Some(&saved), "new").unwrap();
        assert_eq!(p.start, 150);
        assert!(p.gap.is_some());
        for (v, j, n) in [("v2", "j", 200), ("v", "j2", 200), ("v", "j", 50)] {
            let p = resume_position(v, j, 0, n, Some(&saved), "new").unwrap();
            assert_eq!(p.epoch, "new");
            assert_eq!(p.sequence, 1);
            assert!(p.gap.is_some());
        }
    }
}
