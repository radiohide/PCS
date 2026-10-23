//! Lossless sharing of snapshot item arrays. Every observation keeps its original envelope,
//! timestamp, receipt and evidence addresses. Existing inline history is never rewritten.
use anyhow::{ensure, Result};
use rusqlite::{params, Connection};
use serde_json::{json, Value};

const POINTER: &str = "_pcscope_shared_items_v1";

pub(crate) fn ensure_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS snapshot_contents(
        id INTEGER PRIMARY KEY, digest TEXT NOT NULL UNIQUE, items TEXT NOT NULL,
        uses INTEGER NOT NULL CHECK(uses>=0));
      CREATE INDEX IF NOT EXISTS snapshot_contents_unused ON snapshot_contents(uses) WHERE uses=0;
      CREATE TABLE IF NOT EXISTS snapshot_content_refs(
        source TEXT NOT NULL, scope TEXT NOT NULL, run_id TEXT NOT NULL,
        content_id INTEGER NOT NULL REFERENCES snapshot_contents(id),
        PRIMARY KEY(source,scope,run_id)) WITHOUT ROWID;
      CREATE TRIGGER IF NOT EXISTS snapshot_content_delete AFTER DELETE ON snapshots BEGIN
        UPDATE snapshot_contents SET uses=uses-1 WHERE id=(SELECT content_id FROM snapshot_content_refs WHERE source=OLD.source AND scope=OLD.scope AND run_id=OLD.run_id);
        DELETE FROM snapshot_content_refs WHERE source=OLD.source AND scope=OLD.scope AND run_id=OLD.run_id;
        DELETE FROM snapshot_contents WHERE uses=0;
      END;")?;
    Ok(())
}

/// Caller owns the ingest transaction; content, run and reference commit or roll back together.
pub(crate) fn store(conn: &Connection, source: &str, scope: &str, run: &str, payload: &str, unit: i64) -> Result<()> {
    let mut header: Value = serde_json::from_str(payload)?;
    let items = header.as_object_mut().and_then(|h|h.remove("items"))
        .ok_or_else(||anyhow::anyhow!("snapshot items missing"))?;
    ensure!(items.is_array(), "snapshot items must be an array");
    let text=serde_json::to_string(&items)?;
    // Small/empty arrays cost less inline than an extra dictionary row and reference.
    if text.len()<1024 {
        conn.execute("INSERT INTO snapshots(source,scope,run_id,payload,unit_id) VALUES(?,?,?,?,?)",params![source,scope,run,payload,unit])?;
        return Ok(());
    }
    let hash=crate::digest(&text);
    conn.execute("INSERT INTO snapshot_contents(digest,items,uses) VALUES(?,?,0) ON CONFLICT(digest) DO NOTHING",params![hash,text])?;
    let (id,stored):(i64,String)=conn.query_row("SELECT id,items FROM snapshot_contents WHERE digest=?",[hash],|r|Ok((r.get(0)?,r.get(1)?)))?;
    ensure!(stored==text,"snapshot content digest conflict");
    header[POINTER]=json!(id);
    conn.execute("INSERT INTO snapshots(source,scope,run_id,payload,unit_id) VALUES(?,?,?,?,?)",params![source,scope,run,header.to_string(),unit])?;
    conn.execute("INSERT INTO snapshot_content_refs VALUES(?,?,?,?)",params![source,scope,run,id])?;
    conn.execute("UPDATE snapshot_contents SET uses=uses+1 WHERE id=?",[id])?;
    Ok(())
}

pub(crate) fn resolve(conn:&Connection,text:&str)->Result<Value> {
    resolve_bounded(conn,text,None)
}

pub(crate) fn resolve_bounded(conn:&Connection,text:&str,limit:Option<usize>)->Result<Value> {
    let mut value:Value=serde_json::from_str(text)?;
    if let Some(pointer)=value.get(POINTER) {
        let id=pointer.as_i64().filter(|id|*id>0).ok_or_else(||anyhow::anyhow!("invalid snapshot content reference"))?;
        // A truncated/missing content block is an error, never an empty inventory.
        ensure!(value.get("items").is_none(),"ambiguous snapshot content");
        let raw:String=conn.query_row("SELECT CASE WHEN ? IS NULL OR length(CAST(items AS BLOB))<=? THEN items ELSE NULL END FROM snapshot_contents WHERE id=?",
            params![limit.map(|n|n as i64),limit.map(|n|n as i64),id],|r|r.get(0))?;
        let items:Value=serde_json::from_str(&raw)?;
        ensure!(items.is_array(),"invalid shared snapshot items");
        value.as_object_mut().unwrap().remove(POINTER);
        value["items"]=items;
    }
    Ok(value)
}

#[cfg(test)] mod tests {
    use super::*;
    fn database()->Connection {
        let c=Connection::open_in_memory().unwrap();
        c.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE snapshots(source TEXT,scope TEXT,run_id TEXT,payload TEXT,unit_id INTEGER,PRIMARY KEY(source,scope,run_id));").unwrap();
        ensure_schema(&c).unwrap();c
    }
    fn payload(run:usize,content:&str)->Value {json!({"run_id":run.to_string(),"observed_at":format!("time-{run}"),"complete":true,"items":[{"id":"one","data":{"value":content}}]})}
    fn count(c:&Connection,table:&str)->i64 {c.query_row(&format!("SELECT count(*) FROM {table}"),[],|r|r.get(0)).unwrap()}
    fn saved(c:&Connection,run:usize)->String {c.query_row("SELECT payload FROM snapshots WHERE run_id=?",[run.to_string()],|r|r.get(0)).unwrap()}
    #[test] fn a_thousand_observations_share_content_but_preserve_every_envelope() {
        let c=database();let content="unchanged".repeat(1000);let mut original=0;
        for n in 0..1000 {let p=payload(n,&content);original+=p.to_string().len();let tx=c.unchecked_transaction().unwrap();store(&tx,"source","scope",&n.to_string(),&p.to_string(),0).unwrap();tx.commit().unwrap();}
        assert_eq!(count(&c,"snapshots"),1000);assert_eq!(count(&c,"snapshot_contents"),1);
        let stored:i64=c.query_row("SELECT (SELECT sum(length(payload)) FROM snapshots)+(SELECT sum(length(items)) FROM snapshot_contents)",[],|r|r.get(0)).unwrap();
        assert!(stored*20<original as i64,"{stored} / {original}");
        for n in 0..1000 {assert_eq!(resolve(&c,&saved(&c,n)).unwrap(),payload(n,&content));}
        eprintln!("shared snapshot payload bytes: original={original}, stored={stored}");
    }
    #[test] fn changed_reverted_content_survives_pruning_and_vacuum_until_last_reference() {
        let c=database();let a="A".repeat(2048);let b="B".repeat(2048);
        for (n,value) in [(0,&a),(1,&b),(2,&a)] {store(&c,"source","scope",&n.to_string(),&payload(n,value).to_string(),0).unwrap();}
        assert_eq!(count(&c,"snapshot_contents"),2);
        c.execute("DELETE FROM snapshots WHERE run_id='0'",[]).unwrap();
        c.execute_batch("VACUUM").unwrap();
        assert_eq!(resolve(&c,&saved(&c,2)).unwrap(),payload(2,&a));
        c.execute("DELETE FROM snapshots WHERE run_id='2'",[]).unwrap();
        assert_eq!(count(&c,"snapshot_contents"),1);
        assert_eq!(resolve(&c,&saved(&c,1)).unwrap(),payload(1,&b));
        c.execute("DELETE FROM snapshots",[]).unwrap();
        assert_eq!(count(&c,"snapshot_contents"),0);assert_eq!(count(&c,"snapshot_content_refs"),0);
    }
    #[test] fn rollback_conflicts_and_bad_content_never_become_empty_inventories() {
        let c=database();let p=payload(0,&"X".repeat(2048)).to_string();
        {let tx=c.unchecked_transaction().unwrap();store(&tx,"source","scope","0",&p,0).unwrap();}
        assert_eq!(count(&c,"snapshot_contents"),0);
        store(&c,"source","scope","0",&p,0).unwrap();
        {let tx=c.unchecked_transaction().unwrap();assert!(store(&tx,"source","scope","0",&payload(0,&"Y".repeat(2048)).to_string(),0).is_err());}
        assert_eq!(count(&c,"snapshot_contents"),1);
        let header=saved(&c,0);assert!(resolve_bounded(&c,&header,Some(1024)).is_err());
        c.execute("UPDATE snapshot_contents SET items='invalid'",[]).unwrap();assert!(resolve(&c,&header).is_err());
        c.execute_batch("PRAGMA foreign_keys=OFF; DELETE FROM snapshot_contents;").unwrap();assert!(resolve(&c,&header).is_err());
        assert!(resolve(&c,&json!({POINTER:0}).to_string()).is_err());
    }
    #[test] fn legacy_inline_history_is_not_rewritten_during_upgrade() {
        let mut s=crate::Store::open(":memory:").unwrap();let original=payload(0,&"old".repeat(2000)).to_string();
        s.conn.execute("INSERT INTO snapshots VALUES('legacy','scope','0',?,0)",[&original]).unwrap();
        let tx=s.conn.transaction().unwrap();ensure_schema(&tx).unwrap();tx.commit().unwrap();
        assert_eq!(saved(&s.conn,0),original);assert_eq!(resolve(&s.conn,&original).unwrap(),serde_json::from_str::<Value>(&original).unwrap());
        store(&s.conn,"new","scope","1",&payload(1,"small").to_string(),0).unwrap();
        assert_eq!(count(&s.conn,"snapshot_contents"),0);
    }
}
