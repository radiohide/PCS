//! Object-model schema (database schema version 10; display tables live in display.rs).
//!
//! Adds the entities the object model needs — instances, source bindings, field facts and their
//! history, object actions, evidence references, per-scope coverage and projection state — without
//! touching `records`, `snapshots` or user settings, which stay the raw evidence and history input.
//!
//! Three version numbers are kept apart on purpose:
//!   * database schema version — `PRAGMA user_version`, written once per structural change;
//!   * catalog (rule) version   — `MODEL_CATALOG_VERSION`, written here and in `pcscope-contracts`;
//!   * derived-index fingerprint — `model_catalog_v1` in `settings`, rebuilt only when the catalog
//!     changes, following the existing `search_catalog_v1` pattern.
use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;

pub const SCHEMA_VERSION: i64 = 10;
pub(crate) const OBSERVATION_ADDRESS_INDEX: &str = "evidence_refs_observation_address";
/// Settings key holding the fingerprint of the catalog this database was last projected against.
pub const CATALOG_FINGERPRINT_KEY: &str = "model_catalog_v1";

/// The catalog is embedded once, in `pcscope-contracts`, so storage, the adapter layer and the UI
/// all read the same definitions. These re-exports keep the storage-facing call sites unchanged.
pub use pcscope_contracts::model::{catalog, catalog_fingerprint, catalog_version};
/// Convenience alias for the embedded catalog value.
pub fn catalog_value() -> &'static Value {
    catalog()
}

/// Structural tables. Runs once, inside the `user_version < 6` transaction.
pub fn create_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS object_instances(
            instance_id INTEGER PRIMARY KEY AUTOINCREMENT,
            type_id TEXT NOT NULL,category TEXT NOT NULL,
            machine_id TEXT NOT NULL,installation_id TEXT NOT NULL,
            scope TEXT NOT NULL,native_key TEXT NOT NULL,incarnation INTEGER NOT NULL DEFAULT 1,
            display_name TEXT,
            identity_status TEXT NOT NULL,
            lifecycle TEXT NOT NULL,
            created_at TEXT,
            first_observed_at TEXT NOT NULL,last_observed_at TEXT NOT NULL,last_absent_at TEXT,
            UNIQUE(type_id,machine_id,installation_id,scope,native_key,incarnation));
         CREATE INDEX IF NOT EXISTS object_instances_type ON object_instances(type_id,instance_id);
         CREATE INDEX IF NOT EXISTS object_instances_lifecycle ON object_instances(lifecycle,instance_id);
         CREATE INDEX IF NOT EXISTS object_instances_seen ON object_instances(last_observed_at);

         CREATE TABLE IF NOT EXISTS source_bindings(
            binding_id INTEGER PRIMARY KEY AUTOINCREMENT,
            instance_id INTEGER NOT NULL REFERENCES object_instances(instance_id) ON DELETE CASCADE,
            source TEXT NOT NULL,item_id TEXT NOT NULL,scope TEXT NOT NULL,native_record_id TEXT NOT NULL,
            valid_from TEXT NOT NULL,valid_to TEXT,
            UNIQUE(instance_id,source,item_id,scope,native_record_id,valid_from));
         CREATE INDEX IF NOT EXISTS source_bindings_source ON source_bindings(source,item_id,valid_from);
         CREATE INDEX IF NOT EXISTS source_bindings_instance ON source_bindings(instance_id,valid_from);

         CREATE TABLE IF NOT EXISTS object_facts(
            instance_id INTEGER NOT NULL REFERENCES object_instances(instance_id) ON DELETE CASCADE,
            field_id TEXT NOT NULL,field_kind TEXT NOT NULL,
            value TEXT,value_type TEXT NOT NULL,value_status TEXT NOT NULL,
            source_time TEXT,observed_at TEXT NOT NULL,
            derivation TEXT NOT NULL,rule_version TEXT NOT NULL,
            conflict_sources TEXT,
            PRIMARY KEY(instance_id,field_id));
         CREATE INDEX IF NOT EXISTS object_facts_field ON object_facts(field_id,instance_id);

         CREATE TABLE IF NOT EXISTS fact_history(
            instance_id INTEGER NOT NULL REFERENCES object_instances(instance_id) ON DELETE CASCADE,
            field_id TEXT NOT NULL,
            before_value TEXT,before_status TEXT,before_source_time TEXT,
            after_value TEXT,after_status TEXT,after_source_time TEXT,
            observed_from TEXT NOT NULL,observed_until TEXT NOT NULL,
            source TEXT NOT NULL,rule_version TEXT NOT NULL,
            PRIMARY KEY(instance_id,field_id,observed_until));
         CREATE INDEX IF NOT EXISTS fact_history_time ON fact_history(observed_until);

         CREATE TABLE IF NOT EXISTS object_actions(
            action_row INTEGER PRIMARY KEY AUTOINCREMENT,
            instance_id INTEGER REFERENCES object_instances(instance_id) ON DELETE CASCADE,
            unassigned_reason TEXT,
            action_id TEXT NOT NULL,target_type TEXT,title TEXT,
            result TEXT NOT NULL,
            -- The stage of the action, when the source distinguishes one. R04's merge rule compares
            -- it, so two stages of one operation are never deduplicated into a single record.
            phase TEXT,occurrence_key TEXT,source_record TEXT,
            field_id TEXT,before_value TEXT,after_value TEXT,
            occurred_at TEXT,precision TEXT NOT NULL,interval_from TEXT,interval_until TEXT,
            observed_at TEXT NOT NULL,actor TEXT,program TEXT,
            rule_version TEXT NOT NULL,
            CHECK((instance_id IS NULL) <> (unassigned_reason IS NULL)));
         CREATE INDEX IF NOT EXISTS object_actions_instance ON object_actions(instance_id,observed_at,action_row);
         CREATE INDEX IF NOT EXISTS object_actions_time ON object_actions(observed_at,action_row);

         CREATE TABLE IF NOT EXISTS evidence_refs(
            subject_kind TEXT NOT NULL,
            subject_id TEXT NOT NULL,
            evidence_kind TEXT NOT NULL,
            evidence_id TEXT NOT NULL,
            pruned_at TEXT,
            PRIMARY KEY(subject_kind,subject_id,evidence_kind,evidence_id));
         CREATE INDEX IF NOT EXISTS evidence_refs_evidence ON evidence_refs(evidence_kind,evidence_id);
         CREATE INDEX IF NOT EXISTS evidence_refs_pruned ON evidence_refs(pruned_at);

         CREATE TABLE IF NOT EXISTS coverage_scopes(
            source TEXT NOT NULL,scope TEXT NOT NULL,
            configured INTEGER NOT NULL,
            last_result TEXT NOT NULL,
            last_success_at TEXT,last_attempt_at TEXT,
            history_state TEXT NOT NULL,detail TEXT,
            PRIMARY KEY(source,scope));

         CREATE TABLE IF NOT EXISTS projection_state(
            projection TEXT PRIMARY KEY,
            rule_version TEXT NOT NULL,schema_version INTEGER NOT NULL,
            cursor TEXT,watermark TEXT,
            pending INTEGER NOT NULL DEFAULT 0,failed INTEGER NOT NULL DEFAULT 0,
            last_error TEXT,updated_at TEXT NOT NULL);

         CREATE TABLE IF NOT EXISTS type_definitions(
            type_id TEXT PRIMARY KEY,category TEXT NOT NULL,name TEXT NOT NULL,
            identity_strategy TEXT NOT NULL,identity_keys TEXT NOT NULL,supplied_by_source INTEGER NOT NULL);
         CREATE INDEX IF NOT EXISTS type_definitions_category ON type_definitions(category,type_id);
         CREATE TABLE IF NOT EXISTS action_definitions(
            action_id TEXT PRIMARY KEY,name TEXT NOT NULL,nature TEXT NOT NULL,
            result TEXT NOT NULL,target_type TEXT,category TEXT);
         CREATE TABLE IF NOT EXISTS type_actions(
            type_id TEXT NOT NULL,action_id TEXT NOT NULL,PRIMARY KEY(type_id,action_id));
         CREATE TABLE IF NOT EXISTS source_map(
            source TEXT NOT NULL,destination TEXT NOT NULL,types TEXT NOT NULL,note TEXT,
            PRIMARY KEY(source,destination,types));",
    )?;
    Ok(())
}

/// Additive acceleration only: old schema-9 programs still read and maintain the same tables.
/// Writers create it once; readers neither migrate nor require it. CREATE INDEX is an atomic
/// SQLite statement, so interruption or insufficient disk space cannot leave a partial index.
pub(crate) fn ensure_query_indexes(conn: &Connection) -> Result<()> {
    conn.execute_batch(&format!(
        "CREATE INDEX IF NOT EXISTS {OBSERVATION_ADDRESS_INDEX} ON evidence_refs(evidence_id,subject_id) WHERE subject_kind='observation';
         CREATE INDEX IF NOT EXISTS evidence_refs_action_address ON evidence_refs(evidence_id,subject_id) WHERE subject_kind='action';"
    ))?;
    Ok(())
}

/// Catalog-derived lookup tables. Rebuilt only when the catalog fingerprint changes, so repeated
/// startups do not re-migrate and the original evidence is never rewritten by a rule change.
pub fn ensure_catalog(conn: &mut Connection) -> Result<()> {
    let fingerprint = catalog_fingerprint();
    let current: Option<String> = conn
        .query_row("SELECT value FROM settings WHERE key=?", [CATALOG_FINGERPRINT_KEY], |r| r.get(0))
        .optional()?;
    if current.as_deref() == Some(fingerprint.as_str()) {
        return Ok(());
    }
    let types = catalog()["types"].as_array().context("catalog types missing")?;
    let sources = catalog()["sources"].as_array().context("catalog sources missing")?;
    let tx = conn.transaction()?;
    tx.execute_batch("DELETE FROM type_actions;DELETE FROM type_definitions;DELETE FROM action_definitions;DELETE FROM source_map;")?;
    for entry in types {
        let type_id = entry["id"].as_str().context("type id")?;
        let identity = &entry["identity"];
        tx.execute(
            "INSERT INTO type_definitions VALUES(?,?,?,?,?,?)",
            params![
                type_id,
                entry["category"].as_str().context("category")?,
                entry["name"].as_str().context("name")?,
                identity["strategy"].as_str().unwrap_or(""),
                identity["keys"].as_array().map(|k| serde_json::to_string(k).unwrap_or_default()).unwrap_or_default(),
                1,
            ],
        )?;
        if let Some(actions) = entry["actions"].as_array() {
            for action in actions {
                if let Some(action_id) = action.as_str() {
                    tx.execute("INSERT OR IGNORE INTO type_actions VALUES(?,?)", params![type_id, action_id])?;
                }
            }
        }
    }
    for action in catalog()["actions"].as_array().context("catalog actions missing")? {
        let action_id = action["id"].as_str().context("action id")?;
        let category: Option<String> = tx
            .query_row("SELECT category FROM type_definitions WHERE type_id=?", [action["target_type"].as_str().unwrap_or("")], |r| r.get(0))
            .optional()?;
        tx.execute(
            "INSERT INTO action_definitions VALUES(?,?,?,?,?,?)",
            params![
                action_id,
                action["name"].as_str().context("action name")?,
                action["nature"].as_str().context("action nature")?,
                action["result"].as_str().context("action result")?,
                action["target_type"].as_str(),
                category,
            ],
        )?;
    }
    for entry in sources {
        tx.execute(
            "INSERT OR REPLACE INTO source_map VALUES(?,?,?,?)",
            params![
                entry["source"].as_str().context("source")?,
                entry["destination"].as_str().context("destination")?,
                entry["types"].as_array().map(|t| serde_json::to_string(t).unwrap_or_default()).unwrap_or_default(),
                entry["note"].as_str(),
            ],
        )?;
    }
    // A type is fed when at least one observation mapping names it.
    tx.execute(
        "UPDATE type_definitions SET supplied_by_source=EXISTS(
            SELECT 1 FROM source_map m WHERE m.destination='observation' AND m.types LIKE '%\"'||type_id||'\"%')",
        [],
    )?;
    tx.execute(
        "INSERT INTO settings VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![CATALOG_FINGERPRINT_KEY, fingerprint],
    )?;
    tx.commit()?;
    Ok(())
}

/// Projection bookkeeping rows. Idempotent: existing cursors and watermarks are never reset.
pub fn ensure_projections(conn: &Connection, time: &str) -> Result<()> {
    let rule_version = catalog_fingerprint();
    for projection in ["objects", "facts", "actions", "coverage"] {
        conn.execute(
            "INSERT INTO projection_state(projection,rule_version,schema_version,updated_at)
             VALUES(?,?,?,?) ON CONFLICT(projection) DO UPDATE SET
                rule_version=excluded.rule_version,schema_version=excluded.schema_version,updated_at=excluded.updated_at",
            params![projection, rule_version, SCHEMA_VERSION, time],
        )?;
    }
    Ok(())
}

/// Mark references whose evidence is being removed by retention. Facts and actions keep their own
/// value and observation metadata; only the pointer is marked, so a pruned conclusion stays
/// explainable instead of becoming a dangling reference.
pub fn mark_pruned_records(conn: &Connection, unit: i64, time: &str) -> Result<usize> {
    Ok(conn.execute(
        "UPDATE evidence_refs SET pruned_at=? WHERE pruned_at IS NULL AND evidence_kind='record'
            AND CAST(evidence_id AS INTEGER) IN (SELECT seq FROM records WHERE unit_id=?)",
        params![time, unit],
    )?)
}
pub fn mark_pruned_snapshots(conn: &Connection, unit: i64, time: &str) -> Result<usize> {
    Ok(conn.execute(
        "UPDATE evidence_refs SET pruned_at=? WHERE pruned_at IS NULL AND evidence_kind='snapshot'
            AND evidence_id IN (SELECT source||'|'||scope||'|'||run_id FROM snapshots WHERE unit_id=?)",
        params![time, unit],
    )?)
}

/// Remove references whose subject no longer exists. Run after deleting instances, so a removed
/// object never leaves references that nothing can explain.
pub fn delete_orphan_refs(conn: &Connection) -> Result<usize> {
    Ok(conn.execute(
        "DELETE FROM evidence_refs WHERE
            (subject_kind='fact' AND NOT EXISTS(SELECT 1 FROM object_facts f WHERE f.instance_id||'/'||f.field_id=subject_id))
            OR (subject_kind='history' AND NOT EXISTS(SELECT 1 FROM fact_history h WHERE h.instance_id||'/'||h.field_id||'/'||h.observed_until=subject_id))
            OR (subject_kind='action' AND NOT EXISTS(SELECT 1 FROM object_actions a WHERE CAST(a.action_row AS TEXT)=subject_id))
            OR (subject_kind='binding' AND NOT EXISTS(SELECT 1 FROM source_bindings b WHERE CAST(b.binding_id AS TEXT)=subject_id))",
        [],
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Store;
    use std::collections::BTreeMap;

    fn table_names(conn: &Connection) -> Vec<String> {
        let mut q = conn.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name").unwrap();
        q.query_map([], |r| r.get::<_, String>(0)).unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap()
    }
    fn settings_map(conn: &Connection) -> BTreeMap<String, String> {
        let mut q = conn.prepare("SELECT key,value FROM settings ORDER BY key").unwrap();
        q.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))).unwrap().collect::<rusqlite::Result<_>>().unwrap()
    }
    fn temp_path(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("pcscope-model-{}-{}-{}.db", std::process::id(), tag,
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()))
    }
    const NEW_TABLES: [&str; 12] = ["object_instances","source_bindings","object_facts","fact_history","object_actions",
        "evidence_refs","coverage_scopes","projection_state","type_definitions","action_definitions","type_actions","source_map"];

    #[test]
    fn empty_database_reaches_schema_6_and_loads_the_catalog() {
        let store = Store::open(":memory:").unwrap();
        let version: i64 = store.conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        let names = table_names(&store.conn);
        for table in NEW_TABLES { assert!(names.contains(&table.to_string()), "missing {table}"); }
        // Catalog-defined lookup tables mirror the embedded catalog, not a hand-written copy.
        let types: i64 = store.conn.query_row("SELECT count(*) FROM type_definitions", [], |r| r.get(0)).unwrap();
        assert_eq!(types as usize, catalog()["types"].as_array().unwrap().len());
        let actions: i64 = store.conn.query_row("SELECT count(*) FROM action_definitions", [], |r| r.get(0)).unwrap();
        assert_eq!(actions as usize, catalog()["actions"].as_array().unwrap().len());
        let sources: i64 = store.conn.query_row("SELECT count(*) FROM source_map", [], |r| r.get(0)).unwrap();
        assert_eq!(sources as usize, catalog()["sources"].as_array().unwrap().len());
        // Types with no feeding source are marked, so an empty list is never read as complete.
        let unfed: i64 = store.conn.query_row("SELECT count(*) FROM type_definitions WHERE supplied_by_source=0", [], |r| r.get(0)).unwrap();
        assert_eq!(unfed, 0, "all catalog types now have a declared collection source");
    }

    #[test]
    fn v5_database_upgrades_and_preserves_records_snapshots_and_settings() {
        let path = temp_path("upgrade");
        let records_before = {
            let store = Store::open(&path).unwrap();
            store.conn.execute_batch(
                "INSERT INTO records(source,kind,object_id,occurred_at,data) VALUES('pcscope.software','object.changed','fixture','2026-09-11T00:00:00Z','{\"a\":1}');
                 INSERT INTO snapshots VALUES('pcscope.software','scope','run-1','{}',0);
                 UPDATE settings SET value='rolling' WHERE key='retention_mode';").unwrap();
            store.conn.query_row("SELECT source||'|'||kind||'|'||object_id||'|'||occurred_at||'|'||data FROM records ORDER BY seq", [], |r| r.get::<_, String>(0)).unwrap()
        };
        // Simulate a database written by the previous release: drop the new structure, step back.
        // Settings are captured here, after stepping back, so the comparison is against the state a
        // previous release would actually have left behind.
        let settings_before;
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch("PRAGMA foreign_keys=OFF;").unwrap();
            for table in NEW_TABLES { conn.execute_batch(&format!("DROP TABLE IF EXISTS {table};")).unwrap(); }
            conn.execute_batch("DELETE FROM settings WHERE key='model_catalog_v1';PRAGMA user_version=5;").unwrap();
            settings_before = settings_map(&conn);
        }
        let store = Store::open(&path).unwrap();
        let version: i64 = store.conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        for table in NEW_TABLES { assert!(table_names(&store.conn).contains(&table.to_string()), "missing {table}"); }
        let records_after: String = store.conn.query_row("SELECT source||'|'||kind||'|'||object_id||'|'||occurred_at||'|'||data FROM records ORDER BY seq", [], |r| r.get(0)).unwrap();
        assert_eq!(records_before, records_after, "existing records must not be rewritten by the upgrade");
        let snapshots: i64 = store.conn.query_row("SELECT count(*) FROM snapshots", [], |r| r.get(0)).unwrap();
        assert_eq!(snapshots, 1);
        let mut settings_after = settings_map(&store.conn);
        settings_after.remove("model_catalog_v1");
        assert_eq!(settings_before, settings_after, "user settings must be unchanged by the upgrade");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn reopening_does_not_remigrate_or_reload_the_catalog() {
        let path = temp_path("reopen");
        let fingerprint;
        {
            let store = Store::open(&path).unwrap();
            fingerprint = store.conn.query_row("SELECT value FROM settings WHERE key=?", [CATALOG_FINGERPRINT_KEY], |r| r.get::<_, String>(0)).unwrap();
            // A row a rebuild would replace; its presence proves the second open was a no-op.
            store.conn.execute("UPDATE type_definitions SET name='sentinel' WHERE type_id='account.user'", []).unwrap();
        }
        let store = Store::open(&path).unwrap();
        let after: String = store.conn.query_row("SELECT value FROM settings WHERE key=?", [CATALOG_FINGERPRINT_KEY], |r| r.get(0)).unwrap();
        assert_eq!(fingerprint, after);
        let sentinel: String = store.conn.query_row("SELECT name FROM type_definitions WHERE type_id='account.user'", [], |r| r.get(0)).unwrap();
        assert_eq!(sentinel, "sentinel", "reopening with an unchanged catalog must not rebuild the lookup tables");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn observation_index_is_writer_only_idempotent_and_preserves_schema9_data() {
        let path = temp_path("observation-index");
        let before;
        {
            let mut store = Store::open(&path).unwrap();
            store.internal_event("collector.test", "2026-10-13T00:00:00Z", serde_json::json!({"keep":"原始材料"})).unwrap();
            store.conn.execute_batch("DROP INDEX evidence_refs_observation_address; UPDATE type_definitions SET name='sentinel' WHERE type_id='account.user';").unwrap();
            before = settings_map(&store.conn);
        }
        {
            let reader = Store::open_reader(&path, std::time::Duration::from_secs(6)).unwrap();
            assert_eq!(reader.conn.query_row("SELECT count(*) FROM sqlite_schema WHERE name=?", [OBSERVATION_ADDRESS_INDEX], |row| row.get::<_,i64>(0)).unwrap(), 0);
            assert!(reader.query(&pcscope_contracts::query::QueryRequest::InstanceList(Default::default())).is_ok());
            assert_eq!(reader.conn.query_row("PRAGMA user_version", [], |row| row.get::<_,i64>(0)).unwrap(), SCHEMA_VERSION);
        }
        {
            let store = Store::open(&path).unwrap();
            ensure_query_indexes(&store.conn).unwrap();
            assert_eq!(store.conn.query_row("SELECT count(*) FROM sqlite_schema WHERE name=?", [OBSERVATION_ADDRESS_INDEX], |row| row.get::<_,i64>(0)).unwrap(), 1);
            assert_eq!(store.conn.query_row("PRAGMA user_version", [], |row| row.get::<_,i64>(0)).unwrap(), SCHEMA_VERSION);
            assert_eq!(settings_map(&store.conn), before);
            assert_eq!(store.conn.query_row("SELECT data FROM records WHERE kind='collector.test'", [], |row| row.get::<_,String>(0)).unwrap(), "{\"keep\":\"原始材料\"}");
            assert_eq!(store.conn.query_row("SELECT name FROM type_definitions WHERE type_id='account.user'", [], |row| row.get::<_,String>(0)).unwrap(), "sentinel");
            // A writer unaware of the optional index still maintains it through ordinary SQLite.
            store.conn.execute("INSERT INTO evidence_refs VALUES('observation','1','snapshot','kept',NULL)", []).unwrap();
            assert_eq!(store.conn.query_row("SELECT count(*) FROM evidence_refs INDEXED BY evidence_refs_observation_address WHERE subject_kind='observation'", [], |row| row.get::<_,i64>(0)).unwrap(), 1);
        }
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn interrupted_observation_index_creation_leaves_no_partial_index_or_data_change() {
        let store = Store::open(":memory:").unwrap();
        store.conn.execute_batch("DROP INDEX evidence_refs_observation_address;
            WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<2000)
            INSERT INTO evidence_refs SELECT 'observation',CAST(x AS TEXT),'snapshot','shared-'||x,NULL FROM n;").unwrap();
        let before = settings_map(&store.conn);
        store.conn.progress_handler(100, Some(|| true));
        assert!(ensure_query_indexes(&store.conn).is_err());
        store.conn.progress_handler(0, None::<fn() -> bool>);
        assert_eq!(store.conn.query_row("SELECT count(*) FROM sqlite_schema WHERE name=?", [OBSERVATION_ADDRESS_INDEX], |row| row.get::<_,i64>(0)).unwrap(), 0);
        assert_eq!(store.conn.query_row("SELECT count(*) FROM evidence_refs", [], |row| row.get::<_,i64>(0)).unwrap(), 2000);
        assert_eq!(settings_map(&store.conn), before);
        assert_eq!(store.conn.query_row("PRAGMA user_version", [], |row| row.get::<_,i64>(0)).unwrap(), SCHEMA_VERSION);
        ensure_query_indexes(&store.conn).unwrap();
    }

    #[test]
    fn failed_migration_rolls_back_and_leaves_the_previous_version() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE records(seq INTEGER PRIMARY KEY);PRAGMA user_version=5;").unwrap();
        let before = table_names(&conn);
        let attempt = (|| -> Result<()> {
            let tx = conn.unchecked_transaction()?;
            create_schema(&tx)?;
            // Fail after the structure exists but before the version stamp commits.
            tx.execute_batch("CREATE TABLE object_instances(duplicate INTEGER)")?;
            tx.execute_batch(&format!("PRAGMA user_version={};", SCHEMA_VERSION))?;
            tx.commit()?;
            Ok(())
        })();
        assert!(attempt.is_err(), "the duplicate table must fail the migration");
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(version, 5, "a failed migration must not advance the version");
        assert_eq!(before, table_names(&conn), "a failed migration must leave no partial structure");
    }

    #[test]
    fn newer_database_is_refused_by_writer_and_reader() {
        let path = temp_path("newer");
        {
            let store = Store::open(&path).unwrap();
            store.conn.execute_batch("PRAGMA user_version=99;").unwrap();
        }
        let writer = Store::open(&path).err().expect("an old writer must refuse a newer schema");
        assert!(writer.to_string().contains("newer than this application"), "{writer}");
        let reader = Store::open_reader(&path, std::time::Duration::from_secs(2)).err().expect("an old reader must refuse a newer schema");
        assert!(reader.to_string().contains("newer than this application"), "{reader}");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn pruning_marks_references_and_keeps_the_conclusion() {
        let path = temp_path("prune");
        let mut store = Store::open(&path).unwrap();
        store.conn.execute_batch(
            "INSERT INTO retention_units VALUES(1,'2026-09-11T00:00:00Z');
             INSERT INTO object_instances(type_id,category,machine_id,installation_id,scope,native_key,display_name,identity_status,lifecycle,first_observed_at,last_observed_at)
                VALUES('startup.scheduled_task','startup','m','i','scope','\\\\Tasks\\\\A','A','confirmed','active','2026-09-11T00:00:00Z','2026-09-11T00:00:00Z');
             INSERT INTO object_facts VALUES(1,'execute','configuration','cmd.exe','string','known','2026-09-11T00:00:00Z','2026-09-11T00:00:00Z','source_provided','1.0.0',NULL);").unwrap();
        // Bulk history in unit 1, enough to exceed the rolling budget and trigger a prune. The
        // payload must stay valid JSON: the record_search trigger extracts fields from it.
        {
            let tx = store.conn.transaction().unwrap();
            let pad = "x".repeat(24_000);
            for index in 0..600 {
                tx.execute("INSERT INTO records(source,kind,object_id,occurred_at,data,unit_id) VALUES('pcscope.eventlog.system','windows.event','e','2026-09-11T00:00:00Z',?,1)",
                    [format!("{{\"n\":{index},\"pad\":\"{pad}\"}}")]).unwrap();
            }
            tx.commit().unwrap();
        }
        let seq: i64 = store.conn.query_row("SELECT min(seq) FROM records WHERE unit_id=1", [], |r| r.get(0)).unwrap();
        store.conn.execute("INSERT INTO evidence_refs VALUES('fact','1/execute','record',?,NULL)", [seq.to_string()]).unwrap();
        store.set_retention("rolling", crate::MIN_QUOTA).unwrap();
        let pruned: Option<String> = store.conn.query_row("SELECT pruned_at FROM evidence_refs WHERE subject_id='1/execute'", [], |r| r.get(0)).unwrap();
        assert!(pruned.is_some(), "a reference to pruned evidence must be marked, not left dangling");
        let value: String = store.conn.query_row("SELECT value FROM object_facts WHERE field_id='execute'", [], |r| r.get(0)).unwrap();
        assert_eq!(value, "cmd.exe", "the current conclusion survives its pruned evidence");
        let gone: i64 = store.conn.query_row("SELECT count(*) FROM records WHERE unit_id=1", [], |r| r.get(0)).unwrap();
        assert_eq!(gone, 0, "the evidence itself is gone");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn orphan_references_are_removed_with_their_subject() {
        let store = Store::open(":memory:").unwrap();
        store.conn.execute_batch(
            "INSERT INTO object_instances(instance_id,type_id,category,machine_id,installation_id,scope,native_key,display_name,identity_status,lifecycle,first_observed_at,last_observed_at)
                VALUES(1,'account.user','account','m','i','scope','S-1-5-21-1','u','confirmed','active','2026-09-11T00:00:00Z','2026-09-11T00:00:00Z');
             INSERT INTO object_facts VALUES(1,'name','identity','u','string','known',NULL,'2026-09-11T00:00:00Z','source_provided','1.0.0',NULL);
             INSERT INTO evidence_refs VALUES('fact','1/name','record','42',NULL);
             INSERT INTO evidence_refs VALUES('fact','9/gone','record','43',NULL);").unwrap();
        store.conn.execute("DELETE FROM object_instances WHERE instance_id=1", []).unwrap();
        delete_orphan_refs(&store.conn).unwrap();
        let remaining: i64 = store.conn.query_row("SELECT count(*) FROM evidence_refs", [], |r| r.get(0)).unwrap();
        assert_eq!(remaining, 0, "references to a deleted subject must not survive it");
    }

    #[test]
    fn actions_must_be_assigned_or_explicitly_unassigned() {
        let store = Store::open(":memory:").unwrap();
        let result = store.conn.execute(
            "INSERT INTO object_actions(action_id,result,precision,observed_at,rule_version) VALUES('execution.process_started','started','exact','2026-09-11T00:00:00Z','1.0.0')", []);
        assert!(result.is_err(), "an action that is neither assigned nor explained as unassigned is rejected");
    }
}
