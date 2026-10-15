//! Read-only schema report for migration verification.
//!
//! Reports the schema version, the object-model tables and the retained row counts without
//! migrating anything, so the same command can be run before and after an upgrade.
//!
//! usage: schema-info <database>
use anyhow::{bail, Result};
use rusqlite::{Connection, OpenFlags};
use serde_json::json;

const MODEL_TABLES: [&str; 12] = [
    "object_instances", "source_bindings", "object_facts", "fact_history", "object_actions",
    "evidence_refs", "coverage_scopes", "projection_state", "type_definitions", "action_definitions",
    "type_actions", "source_map",
];

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        bail!("usage: schema-info <database>");
    }
    let conn = Connection::open_with_flags(&args[1], OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    let exists = |name: &str| -> Result<bool> {
        Ok(conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?)",
            [name],
            |r| r.get(0),
        )?)
    };
    let count = |table: &str| -> Result<i64> {
        Ok(conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))?)
    };
    let mut tables = Vec::new();
    for name in MODEL_TABLES {
        let present = exists(name)?;
        tables.push(json!({"table": name, "present": present, "rows": if present { count(name)? } else { -1 }}));
    }
    let legacy = ["records", "snapshots", "objects", "settings", "checkpoints", "batch_heads", "event_ids"];
    let mut retained = serde_json::Map::new();
    for name in legacy {
        retained.insert(name.into(), json!(if exists(name)? { count(name)? } else { -1 }));
    }
    let settings: Vec<String> = {
        let mut q = conn.prepare("SELECT key FROM settings ORDER BY key")?;
        let rows = q.query_map([], |r| r.get::<_, String>(0))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    println!("{}", serde_json::to_string_pretty(&json!({
        "database": args[1],
        "schema_version": version,
        "schema_version_supported": pcscope_storage::model::SCHEMA_VERSION,
        "catalog_version": pcscope_storage::model::catalog_version(),
        "model_tables": tables,
        "retained_tables": retained,
        "settings_keys": settings,
    }))?);
    Ok(())
}
