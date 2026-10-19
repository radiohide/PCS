//! Standalone receiver prototype. An in-memory scratch database only; no production integration.
#[path="../src/collection_runs.rs"]
mod collection_runs;
use anyhow::Result;
use collection_runs::*;
use pcscope_contracts::Item;
use rusqlite::Connection;
use serde_json::json;
fn main()->Result<()> {
    let conn=Connection::open_in_memory()?;ensure_schema(&conn)?;
    let execution=begin_execution(&conn)?;configure(&conn,&execution,"fixture","machine:fixture","config-1")?;
    let (manifest,parts)=manifest_for(&[vec![Item{id:"one".into(),data:json!({"enabled":true})}],vec![]],"2026-10-19T01:00:00Z")?;
    let run=allocate(&conn,&execution,"fixture","machine:fixture","config-1",&manifest)?;
    receive_part(&conn,&run,0,&parts[0])?;
    assert!(received_items(&conn,"fixture","machine:fixture")?.is_none());
    seal(&conn,&run)?;
    assert_eq!(receive_part(&conn,&run,1,&parts[1])?,Receipt::Received);
    assert_eq!(received_items(&conn,"fixture","machine:fixture")?.unwrap().len(),1);
    println!("{}",json!({"prototype":"raw_receiver_only","incomplete_hidden":true,"complete_received":true,"production_connected":false}));Ok(())
}
