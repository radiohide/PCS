//! Transactional content-byte accounting. Does not scan old material or pretend logical bytes
//! include SQLite indexes, free pages or WAL. Shared content is charged once to its first source.
use anyhow::Result;
use rusqlite::{params,Connection};
use serde_json::{json,Value};

pub(crate) fn ensure(c:&Connection)->Result<()> {
    c.execute_batch("CREATE TABLE IF NOT EXISTS storage_source_bytes(source TEXT PRIMARY KEY,retained_bytes INTEGER NOT NULL DEFAULT 0,written_bytes INTEGER NOT NULL DEFAULT 0,snapshots INTEGER NOT NULL DEFAULT 0,records INTEGER NOT NULL DEFAULT 0);
      CREATE TABLE IF NOT EXISTS storage_content_owners(content_id INTEGER PRIMARY KEY,source TEXT NOT NULL,bytes INTEGER NOT NULL);
      INSERT OR IGNORE INTO settings SELECT 'storage_accounting_start_unit',CAST(coalesce(max(id),0)+1 AS TEXT) FROM retention_units;
      INSERT OR IGNORE INTO settings VALUES('storage_accounting_started_at',strftime('%Y-%m-%dT%H:%M:%fZ','now'));
      CREATE TRIGGER IF NOT EXISTS storage_record_add AFTER UPDATE OF unit_id ON records WHEN OLD.unit_id=0 AND NEW.unit_id>=CAST((SELECT value FROM settings WHERE key='storage_accounting_start_unit') AS INTEGER) BEGIN
        INSERT INTO storage_source_bytes(source,retained_bytes,written_bytes,records) VALUES(NEW.source,length(CAST(NEW.data AS BLOB)),length(CAST(NEW.data AS BLOB)),1)
          ON CONFLICT(source) DO UPDATE SET retained_bytes=retained_bytes+excluded.retained_bytes,written_bytes=written_bytes+excluded.written_bytes,records=records+1;
      END;
      CREATE TRIGGER IF NOT EXISTS storage_record_remove AFTER DELETE ON records WHEN OLD.unit_id>=CAST((SELECT value FROM settings WHERE key='storage_accounting_start_unit') AS INTEGER) BEGIN
        UPDATE storage_source_bytes SET retained_bytes=retained_bytes-length(CAST(OLD.data AS BLOB)) WHERE source=OLD.source;
      END;
      CREATE TRIGGER IF NOT EXISTS storage_snapshot_add AFTER INSERT ON snapshots WHEN NEW.unit_id>=CAST((SELECT value FROM settings WHERE key='storage_accounting_start_unit') AS INTEGER) BEGIN
        INSERT INTO storage_source_bytes(source,retained_bytes,written_bytes,snapshots) VALUES(NEW.source,length(CAST(NEW.payload AS BLOB)),length(CAST(NEW.payload AS BLOB)),1)
          ON CONFLICT(source) DO UPDATE SET retained_bytes=retained_bytes+excluded.retained_bytes,written_bytes=written_bytes+excluded.written_bytes,snapshots=snapshots+1;
      END;
      CREATE TRIGGER IF NOT EXISTS storage_snapshot_remove AFTER DELETE ON snapshots WHEN OLD.unit_id>=CAST((SELECT value FROM settings WHERE key='storage_accounting_start_unit') AS INTEGER) BEGIN
        UPDATE storage_source_bytes SET retained_bytes=retained_bytes-length(CAST(OLD.payload AS BLOB)) WHERE source=OLD.source;
      END;
      CREATE TRIGGER IF NOT EXISTS storage_content_remove AFTER DELETE ON snapshot_contents BEGIN
        UPDATE storage_source_bytes SET retained_bytes=retained_bytes-coalesce((SELECT bytes FROM storage_content_owners WHERE content_id=OLD.id),0) WHERE source=(SELECT source FROM storage_content_owners WHERE content_id=OLD.id);
        DELETE FROM storage_content_owners WHERE content_id=OLD.id;
      END;")?;Ok(())
}

pub(crate) fn content(c:&Connection,id:i64,source:&str,bytes:usize)->Result<()> {
    // Existing blocks from v11 are outside this accounting window and must not be charged again.
    c.execute("INSERT INTO storage_content_owners VALUES(?,?,?)",params![id,source,bytes as i64])?;
    c.execute("INSERT INTO storage_source_bytes(source,retained_bytes,written_bytes) VALUES(?,?,?) ON CONFLICT(source) DO UPDATE SET retained_bytes=retained_bytes+excluded.retained_bytes,written_bytes=written_bytes+excluded.written_bytes",params![source,bytes as i64,bytes as i64])?;Ok(())
}

pub(crate) fn report(c:&Connection)->Result<Value> {
    let exists:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='storage_source_bytes')",[],|r|r.get(0))?;
    if !exists{return Ok(json!({"available":false}))}
    let start:String=c.query_row("SELECT value FROM settings WHERE key='storage_accounting_started_at'",[],|r|r.get(0))?;
    let mut q=c.prepare_cached("SELECT source,retained_bytes,written_bytes,snapshots,records FROM storage_source_bytes ORDER BY retained_bytes DESC,source")?;
    let mut categories=std::collections::BTreeMap::<String,i64>::new();
    let rows=q.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?,r.get::<_,i64>(2)?,r.get::<_,i64>(3)?,r.get::<_,i64>(4)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let sources:Vec<Value>=rows.into_iter().map(|(source,retained,written,snapshots,records)|{
        let definitions:Vec<_>=crate::collection::definitions().iter().filter(|d|d["source"].as_str()==Some(source.as_str())).collect();
        let category=definitions.first().and_then(|d|d["category"].as_str()).unwrap_or("other");
        let name=if source=="pcscope.shell.registrations"{"资源管理器扩展登记"}else{definitions.iter().find(|d|d["contains"].is_null()).or(definitions.first()).and_then(|d|d["name"].as_str()).unwrap_or("其他采集内容")};
        *categories.entry(category.to_owned()).or_default()+=retained;
        json!({"source":source,"name":name,"category":category,"retained_bytes":retained,"written_bytes":written,"snapshots":snapshots,"records":records})
    }).collect();
    let categories:Vec<_>=categories.into_iter().map(|(category,bytes)|json!({"category":category,"retained_bytes":bytes})).collect();
    Ok(json!({"available":true,"started_at":start,"basis":"retained_content_bytes_since_upgrade","shared_content":"charged_once_to_first_source","sources":sources,"categories":categories}))
}
