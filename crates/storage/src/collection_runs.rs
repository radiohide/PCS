//! Isolated W01/W02 receiver prototype. Not installed by Store::open or used by production yet.
//! A raw receipt is distinct from model/display publication. Host tickets, not message claims,
//! establish execution ownership, ordering and configuration. Original part bytes are immutable.
use anyhow::{bail,ensure,Result};
use pcscope_contracts::Item;
use rusqlite::{params,Connection,OptionalExtension};
use serde::{Deserialize,Serialize};
use sha2::{Digest,Sha256};

const MAX_PART:usize=1_048_576;
const MAX_TOTAL:usize=64*MAX_PART;
#[derive(Clone,Debug)]
pub struct Execution {epoch:String}
#[derive(Clone,Debug)]
pub struct Ticket {id:i64,epoch:String,source:String,scope:String,config:String,seq:i64}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct Manifest {pub parts:usize,pub items:usize,pub bytes:usize,pub digest:String,pub observed_at:String}
#[derive(Debug,PartialEq,Eq)]
pub enum Receipt {Stored,Duplicate,Received,Historical,Cancelled}
pub fn ensure_schema(conn:&Connection)->Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS receiver_execution(id INTEGER PRIMARY KEY CHECK(id=1),epoch TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS collection_config(source TEXT NOT NULL,scope TEXT NOT NULL,revision TEXT NOT NULL,PRIMARY KEY(source,scope));
      CREATE TABLE IF NOT EXISTS collection_scope_heads(source TEXT NOT NULL,scope TEXT NOT NULL,config TEXT NOT NULL,
        issued INTEGER NOT NULL DEFAULT 0,received_run INTEGER,received_seq INTEGER NOT NULL DEFAULT 0,PRIMARY KEY(source,scope,config));
      CREATE TABLE IF NOT EXISTS collection_runs(run_id INTEGER PRIMARY KEY,epoch TEXT NOT NULL,source TEXT NOT NULL,scope TEXT NOT NULL,
        config TEXT NOT NULL,seq INTEGER NOT NULL,state TEXT NOT NULL,manifest TEXT NOT NULL,end_seen INTEGER NOT NULL DEFAULT 0,error TEXT,
        UNIQUE(source,scope,config,seq));
      CREATE TABLE IF NOT EXISTS collection_parts(run_id INTEGER NOT NULL,part INTEGER NOT NULL,digest TEXT NOT NULL,
        item_count INTEGER NOT NULL,byte_count INTEGER NOT NULL,original BLOB NOT NULL,PRIMARY KEY(run_id,part));
      CREATE TABLE IF NOT EXISTS collection_item_keys(run_id INTEGER NOT NULL,item_id TEXT NOT NULL,part INTEGER NOT NULL,
        PRIMARY KEY(run_id,item_id));")?;Ok(())
}
pub fn begin_execution(conn:&Connection)->Result<Execution>{
    let tx=conn.unchecked_transaction()?;
    let epoch:String=tx.query_row("SELECT lower(hex(randomblob(16)))",[],|r|r.get(0))?;
    tx.execute("INSERT INTO receiver_execution VALUES(1,?) ON CONFLICT(id) DO UPDATE SET epoch=excluded.epoch",[&epoch])?;
    tx.execute("UPDATE collection_runs SET state='abandoned',error='execution replaced' WHERE state IN ('collecting','superseded')",[])?;
    tx.commit()?;Ok(Execution{epoch})
}
pub fn configure(conn:&Connection,execution:&Execution,source:&str,scope:&str,revision:&str)->Result<()> {
    ensure!(!source.is_empty()&&source.len()<=128&&!scope.is_empty()&&scope.len()<=4096&&!revision.is_empty()&&revision.len()<=128,"invalid collection scope");
    let tx=conn.unchecked_transaction()?;ensure!(current_epoch(&tx)?==execution.epoch,"stale host execution");
    let previous:Option<String>=tx.query_row("SELECT revision FROM collection_config WHERE source=? AND scope=?",params![source,scope],|r|r.get(0)).optional()?;
    let used:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM collection_scope_heads WHERE source=? AND scope=? AND config=?)",params![source,scope,revision],|r|r.get(0))?;
    ensure!(!used||previous.as_deref()==Some(revision),"retired configuration revision cannot be reused");
    tx.execute("INSERT INTO collection_config VALUES(?,?,?) ON CONFLICT(source,scope) DO UPDATE SET revision=excluded.revision",params![source,scope,revision])?;
    tx.execute("UPDATE collection_runs SET state='cancelled',error='configuration replaced' WHERE source=? AND scope=? AND config<>? AND state IN ('collecting','superseded')",params![source,scope,revision])?;
    tx.commit()?;Ok(())
}
fn current_epoch(conn:&Connection)->Result<String>{Ok(conn.query_row("SELECT epoch FROM receiver_execution WHERE id=1",[],|r|r.get(0))?)}
fn current_config(conn:&Connection,source:&str,scope:&str)->Result<String>{Ok(conn.query_row("SELECT revision FROM collection_config WHERE source=? AND scope=?",params![source,scope],|r|r.get(0))?)}
fn manifest_valid(manifest:&Manifest)->Result<()> {
    ensure!((1..=64).contains(&manifest.parts)&&manifest.items<=50000&&manifest.bytes>=manifest.parts*2&&manifest.bytes<=MAX_TOTAL,"collection manifest exceeds budget");
    ensure!(manifest.digest.len()==64&&manifest.digest.bytes().all(|b|b.is_ascii_hexdigit()),"invalid collection digest");
    pcscope_contracts::semantics::canonical_time(&manifest.observed_at).map_err(anyhow::Error::msg)?;Ok(())
}
pub fn allocate(conn:&Connection,execution:&Execution,source:&str,scope:&str,config:&str,manifest:&Manifest)->Result<Ticket> {
    manifest_valid(manifest)?;let tx=conn.unchecked_transaction()?;
    ensure!(current_epoch(&tx)?==execution.epoch&&current_config(&tx,source,scope)?==config,"stale execution or configuration");
    tx.execute("INSERT OR IGNORE INTO collection_scope_heads(source,scope,config) VALUES(?,?,?)",params![source,scope,config])?;
    let issued:i64=tx.query_row("SELECT issued FROM collection_scope_heads WHERE source=? AND scope=? AND config=?",params![source,scope,config],|r|r.get(0))?;
    let seq=issued.checked_add(1).filter(|seq|*seq>0).ok_or_else(||anyhow::anyhow!("collection sequence exhausted"))?;
    tx.execute("UPDATE collection_scope_heads SET issued=? WHERE source=? AND scope=? AND config=?",params![seq,source,scope,config])?;
    tx.execute("UPDATE collection_runs SET state='superseded' WHERE source=? AND scope=? AND config=? AND state='collecting'",params![source,scope,config])?;
    tx.execute("INSERT INTO collection_runs(epoch,source,scope,config,seq,state,manifest) VALUES(?,?,?,?,?,'collecting',?)",params![execution.epoch,source,scope,config,seq,serde_json::to_string(manifest)?])?;
    let ticket=Ticket{id:tx.last_insert_rowid(),epoch:execution.epoch.clone(),source:source.into(),scope:scope.into(),config:config.into(),seq};
    tx.commit()?;Ok(ticket)
}
fn active(conn:&Connection,ticket:&Ticket)->Result<bool>{
    Ok(current_epoch(conn)?==ticket.epoch&&current_config(conn,&ticket.source,&ticket.scope)?==ticket.config)
}
fn saved(conn:&Connection,ticket:&Ticket)->Result<(String,Manifest,bool)> {
    let (epoch,source,scope,config,seq,state,manifest,end):(String,String,String,String,i64,String,String,bool)=conn.query_row(
        "SELECT epoch,source,scope,config,seq,state,manifest,end_seen FROM collection_runs WHERE run_id=?",[ticket.id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?)))?;
    ensure!((epoch,source,scope,config,seq)==(ticket.epoch.clone(),ticket.source.clone(),ticket.scope.clone(),ticket.config.clone(),ticket.seq),"ticket identity mismatch");
    Ok((state,serde_json::from_str(&manifest)?,end))
}
fn reject(conn:&Connection,ticket:&Ticket,reason:&str)->Result<()> {
    conn.execute("UPDATE collection_runs SET state='rejected',error=? WHERE run_id=?",params![reason,ticket.id])?;Ok(())
}
pub fn receive_part(conn:&Connection,ticket:&Ticket,part:usize,original:&[u8])->Result<Receipt>{
    ensure!(original.len()<=MAX_PART,"part exceeds transport budget");
    let parsed:Vec<Item>=serde_json::from_slice(original)?;
    let mut keys=std::collections::HashSet::new();ensure!(parsed.iter().all(|item|!item.id.is_empty()&&keys.insert(&item.id)),"duplicate or empty item identity");
    let digest=format!("{:x}",Sha256::digest(original));let tx=conn.unchecked_transaction()?;
    if !active(&tx,ticket)?{return Ok(Receipt::Cancelled)}
    let (state,manifest,_)=saved(&tx,ticket)?;
    if matches!(state.as_str(),"cancelled"|"abandoned"|"rejected"){return Ok(Receipt::Cancelled)}
    ensure!(part<manifest.parts,"part outside manifest");
    let previous:Option<Vec<u8>>=tx.query_row("SELECT original FROM collection_parts WHERE run_id=? AND part=?",params![ticket.id,part as i64],|r|r.get(0)).optional()?;
    if let Some(previous)=previous {
        if previous==original{return Ok(Receipt::Duplicate)}
        // A complete receipt is immutable. Conflicting retries cannot revoke published material.
        if !matches!(state.as_str(),"received"|"historical"){reject(&tx,ticket,"part identity reused with different bytes")?;tx.commit()?;}
        bail!("part identity reused with different bytes");
    }
    ensure!(!matches!(state.as_str(),"received"|"historical"),"completed collection is immutable");
    let (bytes,items):(i64,i64)=tx.query_row("SELECT coalesce(sum(byte_count),0),coalesce(sum(item_count),0) FROM collection_parts WHERE run_id=?",[ticket.id],|r|Ok((r.get(0)?,r.get(1)?)))?;
    ensure!(bytes as usize+original.len()<=manifest.bytes&&items as usize+parsed.len()<=manifest.items,"part totals exceed manifest");
    for item in &parsed {
        let duplicate:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM collection_item_keys WHERE run_id=? AND item_id=?)",params![ticket.id,item.id],|r|r.get(0))?;
        if duplicate {reject(&tx,ticket,"item identity repeated across parts")?;tx.commit()?;bail!("item identity repeated across parts")}
    }
    tx.execute("INSERT INTO collection_parts VALUES(?,?,?,?,?,?)",params![ticket.id,part as i64,digest,parsed.len() as i64,original.len() as i64,original])?;
    for item in &parsed {tx.execute("INSERT INTO collection_item_keys VALUES(?,?,?)",params![ticket.id,item.id,part as i64])?;}
    let receipt=try_receive(&tx,ticket)?;tx.commit()?;Ok(receipt)
}
pub fn seal(conn:&Connection,ticket:&Ticket)->Result<Receipt>{
    let tx=conn.unchecked_transaction()?;if !active(&tx,ticket)?{return Ok(Receipt::Cancelled)}
    let (state,_,_)=saved(&tx,ticket)?;
    if matches!(state.as_str(),"received"|"historical"){return Ok(Receipt::Duplicate)}
    if matches!(state.as_str(),"cancelled"|"abandoned"|"rejected"){return Ok(Receipt::Cancelled)}
    tx.execute("UPDATE collection_runs SET end_seen=1 WHERE run_id=?",[ticket.id])?;
    let receipt=try_receive(&tx,ticket)?;tx.commit()?;Ok(receipt)
}
fn parts_digest(digests:&[(usize,String)])->String {
    let mut hash=Sha256::new();hash.update(b"pcscope.snapshot-parts.v2\0");
    for (part,digest) in digests{hash.update((*part as u64).to_le_bytes());hash.update(digest.as_bytes());}
    format!("{:x}",hash.finalize())
}
fn try_receive(conn:&Connection,ticket:&Ticket)->Result<Receipt>{
    let (state,manifest,end)=saved(conn,ticket)?;
    if !end{return Ok(Receipt::Stored)}
    let parts={let mut query=conn.prepare("SELECT part,digest,item_count,byte_count FROM collection_parts WHERE run_id=? ORDER BY part")?;
        let rows=query.query_map([ticket.id],|r|Ok((r.get::<_,usize>(0)?,r.get::<_,String>(1)?,r.get::<_,usize>(2)?,r.get::<_,usize>(3)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;rows};
    if parts.len()!=manifest.parts{return Ok(Receipt::Stored)}
    let digest=parts_digest(&parts.iter().map(|(part,digest,_,_)|(*part,digest.clone())).collect::<Vec<_>>());
    if digest!=manifest.digest||parts.iter().map(|p|p.2).sum::<usize>()!=manifest.items||parts.iter().map(|p|p.3).sum::<usize>()!=manifest.bytes {
        reject(conn,ticket,"complete collection does not match manifest")?;return Ok(Receipt::Cancelled)
    }
    let issued:i64=conn.query_row("SELECT issued FROM collection_scope_heads WHERE source=? AND scope=? AND config=?",params![ticket.source,ticket.scope,ticket.config],|r|r.get(0))?;
    let current=issued==ticket.seq&&state!="superseded";
    conn.execute("UPDATE collection_runs SET state=? WHERE run_id=?",params![if current{"received"}else{"historical"},ticket.id])?;
    if current {conn.execute("UPDATE collection_scope_heads SET received_run=?,received_seq=? WHERE source=? AND scope=? AND config=? AND received_seq<?",params![ticket.id,ticket.seq,ticket.source,ticket.scope,ticket.config,ticket.seq])?;}
    Ok(if current{Receipt::Received}else{Receipt::Historical})
}
/// Scratch diagnostics only. Production integration must consume one bounded part at a time.
pub fn received_items(conn:&Connection,source:&str,scope:&str)->Result<Option<Vec<Item>>>{
    let mut query=conn.prepare("SELECT p.original FROM collection_scope_heads h
        JOIN collection_config c ON c.source=h.source AND c.scope=h.scope AND c.revision=h.config
        JOIN collection_parts p ON p.run_id=h.received_run WHERE h.source=? AND h.scope=? ORDER BY p.part")?;
    let bodies=query.query_map(params![source,scope],|r|r.get::<_,Vec<u8>>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
    if bodies.is_empty(){return Ok(None)}
    let mut items=Vec::new();for body in bodies{items.extend(serde_json::from_slice::<Vec<Item>>(&body)?);}Ok(Some(items))
}
pub fn manifest_for(parts:&[Vec<Item>],observed_at:&str)->Result<(Manifest,Vec<Vec<u8>>)> {
    let bodies=parts.iter().map(serde_json::to_vec).collect::<serde_json::Result<Vec<_>>>()?;
    let digest=parts_digest(&bodies.iter().enumerate().map(|(part,body)|(part,format!("{:x}",Sha256::digest(body)))).collect::<Vec<_>>());
    let manifest=Manifest{parts:parts.len(),items:parts.iter().map(Vec::len).sum(),bytes:bodies.iter().map(Vec::len).sum(),digest,observed_at:observed_at.into()};
    manifest_valid(&manifest)?;Ok((manifest,bodies))
}

#[cfg(test)]
#[path="collection_runs_tests.rs"]
mod tests;
