mod self_activity;
mod timeline_policy;
mod display;
mod display_counts;
#[cfg(test)] mod read_index_tests;
mod registration_dictionary;
mod item_controls;
mod related;
mod collection;
mod coverage;
mod search_index;
pub mod evidence;
pub mod identity;
pub mod projection;
pub mod reconcile;
pub mod faults;
pub mod flow;
pub mod query;
pub mod model;
use anyhow::{bail, Context, Result};
use pcscope_contracts::adapter::EvidenceInput;
use pcscope_contracts::{Envelope, Message};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

pub struct Store {
    conn: Connection,
    path: Option<PathBuf>,
}
/// Turns a full store into something the user can act on.
fn explain_full_store(error: anyhow::Error) -> anyhow::Error {
    if let Some(rusqlite::Error::SqliteFailure(code, _)) = error.downcast_ref::<rusqlite::Error>() {
        if code.code == rusqlite::ErrorCode::DiskFull {
            return anyhow::anyhow!(
                "已达到配置的容量上限，本批材料未能保存；采集端可以重发。请在设置页「历史存储」提高配额，或改用无限增长模式。"
            );
        }
    }
    error
}

fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
/// Stores a complete snapshot's material and indexes it.
///
/// Shared by ingestion and by the migration (S27), so the two cannot disagree about what storing a
/// snapshot means. It writes the material once, one evidence address per item pointing at it, the
/// receipt that makes a replay of the same run a no-op, and the scope record.
#[allow(clippy::too_many_arguments)]
fn store_snapshot(tx: &Connection, source: &str, scope: &str, run_id: &str, payload: &str, unit_id: i64, observed_at: &str, ingested_at: &str, items: &[pcscope_contracts::Item]) -> Result<()> {
    tx.execute(
        "INSERT INTO snapshots(source,scope,run_id,payload,unit_id) VALUES(?,?,?,?,?)",
        params![source, scope, run_id, payload, unit_id],
    )?;
    evidence::record_snapshot_items(tx, source, scope, run_id, observed_at, ingested_at, items, &evidence::decoding_of_stored_material())?;
    tx.execute(
        "INSERT INTO snapshot_receipts VALUES(?,?,?,?)",
        params![source, scope, run_id, digest(payload)],
    )?;
    tx.execute("INSERT OR IGNORE INTO scopes VALUES(?,?)", params![source, scope])?;
    Ok(())
}

/// Whether a source is switched on right now.
///
/// Coverage is per source and scope; the control is per collection item. Asking the control set the
/// source-level question is what lets the coverage row say a scope was off rather than merely
/// unheard from — the two look identical in the data and mean opposite things.
fn source_configured(disabled: &BTreeSet<String>, source: &str) -> bool {
    !disabled.contains(&item_controls::classify(source, "", "", &Value::Null))
}
const DEFAULT_QUOTA: u64 = 5_000_000_000;
const MIN_QUOTA: u64 = 32 * 1024 * 1024;

impl Store {
    /// Online additive maintenance for an existing native database. Does not migrate, ingest,
    /// reset display cursors or rewrite history. SQLite maintains these indexes for old writers.
    pub fn install_read_indexes(path: impl AsRef<Path>) -> Result<Value> {
        let mut conn=Connection::open_with_flags(path,rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let version:i64=conn.query_row("PRAGMA user_version",[],|r|r.get(0))?;
        anyhow::ensure!(version==10,"online index maintenance requires schema 10");
        let marker:Option<String>=conn.query_row("SELECT value FROM settings WHERE key='native_program_identity'",[],|r|r.get(0)).optional()?;
        anyhow::ensure!(marker.as_deref()==Some("pcscope-native-v1"),"refusing to modify an unknown database");
        let tx=conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        evidence::ensure_query_indexes(&tx)?;
        model::ensure_query_indexes(&tx)?;
        tx.commit()?;
        Ok(json!({"schema_version":version,"indexes_ready":true,"history_preserved":true,"display_progress_preserved":true}))
    }

    /// Production entry: a pre-existing unmarked database is never migrated or written.
    /// Store::open remains available to isolated legacy fixtures and explicit diagnostic tools.
    pub fn open_program(path:impl AsRef<Path>)->Result<Self> {
        let path=path.as_ref();let metadata=match std::fs::symlink_metadata(path){Ok(m)=>Some(m),Err(e)if e.kind()==std::io::ErrorKind::NotFound=>None,Err(e)=>return Err(e.into())};let existing=metadata.is_some();
        if existing {
            let metadata=metadata.unwrap();anyhow::ensure!(metadata.is_file()&&!metadata.file_type().is_symlink(),"数据文件必须是普通文件；请为新程序选择独立空目录");
            #[cfg(windows)] {
                use std::os::windows::{fs::MetadataExt,io::AsRawHandle};
                use windows_sys::Win32::Storage::FileSystem::{GetFileInformationByHandle,BY_HANDLE_FILE_INFORMATION};
                anyhow::ensure!(metadata.file_attributes()&0x400==0,"数据文件不能是重解析目标；请使用独立普通文件");
                let file=std::fs::File::open(path)?;let mut info:BY_HANDLE_FILE_INFORMATION=unsafe{std::mem::zeroed()};
                if unsafe{GetFileInformationByHandle(file.as_raw_handle(),&mut info)}==0{return Err(std::io::Error::last_os_error().into());}
                anyhow::ensure!(info.nNumberOfLinks==1,"拒绝共享硬链接数据文件；不同实例必须使用各自的普通数据库");
            }
            let reader=Connection::open_with_flags(path,rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
            let marker:Option<String>=reader.query_row("SELECT value FROM settings WHERE key='native_program_identity'",[],|r|r.get(0)).optional().context("数据格式不属于当前新程序；请选择独立空目录，原库未升级")?;
            anyhow::ensure!(marker.as_deref()==Some("pcscope-native-v1"),"拒绝写入旧库或未知数据文件；请选择独立空目录，原库未升级");
        }
        let store=Self::open(path)?;
        if !existing {store.conn.execute("INSERT INTO settings VALUES('native_program_identity','pcscope-native-v1')",[])?;store.conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;}
        Ok(store)
    }
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let mut conn = Connection::open(path)?;
        conn.set_prepared_statement_cache_capacity(64);
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let version: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        // Refuse rather than write: an older binary must never touch a newer structure. Reading a
        // newer database is refused too, rather than reporting a partially understood model.
        if i64::from(version) > model::SCHEMA_VERSION {
            bail!(
                "database schema version {version} is newer than this application ({}); upgrade the application",
                model::SCHEMA_VERSION
            );
        }
        conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
            BEGIN;
            CREATE TABLE IF NOT EXISTS records(seq INTEGER PRIMARY KEY AUTOINCREMENT,source TEXT NOT NULL,kind TEXT NOT NULL,object_id TEXT NOT NULL,occurred_at TEXT NOT NULL,data TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS event_ids(source TEXT NOT NULL,epoch TEXT NOT NULL,event_id TEXT NOT NULL,payload TEXT NOT NULL,PRIMARY KEY(source,epoch,event_id));
            CREATE TABLE IF NOT EXISTS checkpoints(source TEXT NOT NULL,epoch TEXT NOT NULL,value TEXT NOT NULL,PRIMARY KEY(source,epoch));
            CREATE TABLE IF NOT EXISTS batch_heads(source TEXT NOT NULL,epoch TEXT NOT NULL,seq INTEGER NOT NULL,payload TEXT NOT NULL,PRIMARY KEY(source,epoch));
            CREATE TABLE IF NOT EXISTS gaps(source TEXT NOT NULL,id TEXT NOT NULL,payload TEXT NOT NULL,PRIMARY KEY(source,id));
            CREATE TABLE IF NOT EXISTS objects(source TEXT NOT NULL,scope TEXT NOT NULL,id TEXT NOT NULL,data TEXT NOT NULL,PRIMARY KEY(source,scope,id));
            CREATE TABLE IF NOT EXISTS snapshots(source TEXT NOT NULL,scope TEXT NOT NULL,run_id TEXT NOT NULL,payload TEXT NOT NULL,PRIMARY KEY(source,scope,run_id));
            CREATE INDEX IF NOT EXISTS records_source ON records(source,seq);
            COMMIT;")?;
        if version < 3 {
            conn.execute_batch("PRAGMA auto_vacuum=INCREMENTAL; VACUUM;")?;
            let tx = conn.transaction()?;
            tx.execute_batch("ALTER TABLE records ADD COLUMN unit_id INTEGER NOT NULL DEFAULT 0;
                ALTER TABLE snapshots ADD COLUMN unit_id INTEGER NOT NULL DEFAULT 0;
                CREATE TABLE retention_units(id INTEGER PRIMARY KEY AUTOINCREMENT,created_at TEXT NOT NULL);
                INSERT INTO retention_units VALUES(0,strftime('%Y-%m-%dT%H:%M:%fZ','now'));
                CREATE TABLE scopes(source TEXT NOT NULL,scope TEXT NOT NULL,PRIMARY KEY(source,scope));
                INSERT OR IGNORE INTO scopes SELECT source,scope FROM snapshots;
                CREATE TABLE snapshot_receipts(source TEXT NOT NULL,scope TEXT NOT NULL,run_id TEXT NOT NULL,digest TEXT NOT NULL,PRIMARY KEY(source,scope,run_id));
                CREATE TABLE settings(key TEXT PRIMARY KEY,value TEXT NOT NULL);
                INSERT INTO settings VALUES('retention_mode','unlimited'),('quota_bytes','5000000000'),('pruned_units','0');
                CREATE INDEX records_unit ON records(unit_id);
                CREATE INDEX snapshots_unit ON snapshots(unit_id);
                PRAGMA user_version=3;")?;
            let events: Vec<(String, String, String, String)> = {
                let mut q = tx.prepare("SELECT source,epoch,event_id,payload FROM event_ids")?;
                let rows = q.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
                rows.collect::<rusqlite::Result<_>>()?
            };
            for (source, epoch, id, payload) in events {
                tx.execute(
                    "UPDATE event_ids SET payload=? WHERE source=? AND epoch=? AND event_id=?",
                    params![digest(&payload), source, epoch, id],
                )?;
            }
            let snapshots: Vec<(String, String, String, String)> = {
                let mut q = tx.prepare("SELECT source,scope,run_id,payload FROM snapshots")?;
                let rows = q.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
                rows.collect::<rusqlite::Result<_>>()?
            };
            for (source, scope, run, payload) in snapshots {
                tx.execute(
                    "INSERT INTO snapshot_receipts VALUES(?,?,?,?)",
                    params![source, scope, run, digest(&payload)],
                )?;
            }
            tx.commit()?;
        }
        if version < 4 {
            conn.execute_batch("BEGIN;
                CREATE TABLE object_times(source TEXT NOT NULL,scope TEXT NOT NULL,id TEXT NOT NULL,changed_at TEXT,seen_at TEXT,PRIMARY KEY(source,scope,id));
                CREATE INDEX object_times_changed ON object_times(changed_at);
                INSERT INTO object_times(source,scope,id) SELECT source,scope,id FROM objects;
                PRAGMA user_version=4;
                COMMIT;")?;
        }
        if version < 6 {
            // Structure and version stamp commit together: a failure part way through rolls back,
            // leaving the database at its previous version rather than half migrated.
            //
            // Each block stamps the version it actually migrated to. Stamping the current constant
            // here would make a v5 database skip every later block, so it would report version 7
            // while still missing the tables v7 adds — a hole that stays invisible until something
            // reads them.
            let tx = conn.unchecked_transaction()?;
            model::create_schema(&tx)?;
            tx.execute_batch("PRAGMA user_version=6;")?;
            tx.commit()?;
        }
        if version < 7 {
            let tx = conn.unchecked_transaction()?;
            evidence::ensure(&tx)?;
            tx.execute_batch("PRAGMA user_version=7;")?;
            tx.commit()?;
        }
        if version < 8 {
            // S04's structure gave `object_actions` no way to record which stage an action belongs
            // to, although R04's merge rule compares stages — a start and an end are two records and
            // must never be collapsed into one. A database created by the current `create_schema`
            // already has the column, so it is added only where it is missing.
            let tx = conn.unchecked_transaction()?;
            let has_phase = {
                let mut statement = tx.prepare("PRAGMA table_info(object_actions)")?;
                let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
                let names: Vec<String> = rows.filter_map(std::result::Result::ok).collect();
                names.iter().any(|name| name == "phase")
            };
            if !has_phase {
                tx.execute_batch("ALTER TABLE object_actions ADD COLUMN phase TEXT;")?;
            }
            tx.execute_batch("PRAGMA user_version=8;")?;
            tx.commit()?;
        }
        if version < 9 {
            let tx=conn.unchecked_transaction()?;
            let columns={let mut s=tx.prepare("PRAGMA table_info(object_actions)")?;let names=s.query_map([],|row|row.get::<_,String>(1))?.collect::<rusqlite::Result<Vec<_>>>()?;names};
            for column in ["occurrence_key","source_record"] { if !columns.iter().any(|name|name==column) { tx.execute_batch(&format!("ALTER TABLE object_actions ADD COLUMN {column} TEXT;"))?; } }
            tx.execute_batch("CREATE UNIQUE INDEX IF NOT EXISTS object_actions_occurrence ON object_actions(occurrence_key) WHERE occurrence_key IS NOT NULL; PRAGMA user_version=9;")?;
            tx.commit()?;
        }
        search_index::ensure(&mut conn)?;
        self_activity::ensure(&conn)?;
        coverage::ensure(&conn)?;
        evidence::ensure_query_indexes(&conn)?;
        coverage::ensure_projection_counts(&conn)?;
        model::ensure_query_indexes(&conn)?;
        {
            let tx=conn.unchecked_transaction()?;
            display::ensure(&tx)?;
            display_counts::ensure(&tx)?;
            tx.execute_batch("PRAGMA user_version=10;")?;
            tx.commit()?;
        }
        // How deep the projection backlog may get before ingest stops accepting more. A default
        // rather than a constant so a machine that collects faster than it projects can be tuned
        // without a rebuild.
        conn.execute("INSERT OR IGNORE INTO settings VALUES('max_pending_projections','200000')", [])?;
        // Catalog-dependent tables rebuild only when the catalog fingerprint changes, so a restart
        // is a no-op and a rule change never rewrites the original evidence.
        model::ensure_catalog(&mut conn)?;
        let now: String = conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ','now')", [], |r| r.get(0))?;
        model::ensure_projections(&conn, &now)?;
        timeline_policy::install(&conn)?;
        let mut store = Self {
            conn,
            path: if path == Path::new(":memory:") {
                None
            } else {
                Some(path.to_path_buf())
            },
        };
        store.maintain_capacity(0)?;
        Ok(store)
    }

    /// Independent WAL reader: never takes the collection writer's mutex or performs migrations.
    pub fn open_reader(path: impl AsRef<Path>, budget: std::time::Duration) -> Result<Self> {
        let conn=Connection::open_with_flags(path,rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY|rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
        conn.set_prepared_statement_cache_capacity(64);
        timeline_policy::install(&conn)?;
        // Readers refuse a newer structure too; a partially understood model must not be presented
        // as if it were complete.
        let version:i64=conn.query_row("PRAGMA user_version",[],|r|r.get(0))?;
        if version>model::SCHEMA_VERSION {bail!("database schema version {version} is newer than this application ({})",model::SCHEMA_VERSION);}
        conn.busy_timeout(std::time::Duration::from_millis(250))?;
        conn.execute_batch("PRAGMA query_only=ON;")?;
        let deadline=std::time::Instant::now()+budget;
        conn.progress_handler(1000,Some(move ||std::time::Instant::now()>=deadline));
        Ok(Self {conn,path:None})
    }

    /// Only return success after transaction commit; caller may then acknowledge.
    /// Ingests one envelope, reporting a full store in the store's own terms.
    ///
    /// The quota is enforced with a page limit, so exceeding it surfaces as the storage engine's
    /// `SQLITE_FULL` from somewhere inside an insert — a message that says neither which limit was
    /// reached nor what to do about it, and that reads like a corrupt disk rather than a setting.
    /// The collection did not happen either way; the point is that the reader can tell why.
    pub fn ingest(&mut self, envelope: &Envelope) -> Result<usize> {
        self.ingest_unchecked(envelope, false).map_err(explain_full_store)
    }

    fn ingest_unchecked(&mut self, envelope: &Envelope, control_plane: bool) -> Result<usize> {
        let Some(selected)=self.select_envelope(envelope)? else{return Ok(0);};
        let envelope=&selected;
        let disabled_items=self.disabled_items()?;
        // Validation is also enforced for in-process callers.
        let encoded = serde_json::to_string(envelope)?;
        let checked = pcscope_contracts::parse(&encoded)?;
        if matches!(
            checked.message,
            Message::Snapshot {
                complete: false,
                ..
            }
        ) {
            bail!("incomplete snapshot: current state and history unchanged");
        }
        if self.preflight(&checked)? {
            return Ok(0);
        }
        // A projection that cannot keep up must not consume the disk silently. Refusing the batch
        // rolls it back entirely, so nothing is lost: the source still holds the material and can
        // re-send it. The alternative — accepting forever and pruning the backlog itself — would
        // throw away the raw evidence the object model is built on.
        let backlog: i64 = self.conn.query_row("SELECT count(*) FROM projection_queue WHERE state='pending'", [], |r| r.get(0))?;
        let max_pending: i64 = self.setting("max_pending_projections")?.parse()?;
        if !control_plane && backlog > max_pending {
            bail!("projection backlog is {backlog} items, above the limit of {max_pending}; run the projection or raise max_pending_projections");
        }
        self.maintain_capacity(encoded.len() as u64 * 8 + 262144)?;
        let tx = self.conn.transaction()?;
        let previous_seq: i64 =
            tx.query_row("SELECT coalesce(max(seq),0) FROM records", [], |r| r.get(0))?;
        tx.execute(
            "INSERT INTO retention_units(created_at) VALUES(strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
            [],
        )?;
        let unit_id = tx.last_insert_rowid();
        // When this material reached the database, recorded per batch so every item of one batch
        // shares it. It is a different question from when the source says something happened and
        // from when PCScope saw it, and is kept in its own column rather than inferred later.
        let ingested_at: String = tx.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ','now')", [], |r| r.get(0))?;
        let decoding = evidence::decoding_of_stored_material();
        let mut inserted = 0;
        match &checked.message {
            Message::EventBatch {
                source,
                epoch,
                batch_seq,
                checkpoint,
                events,
            } => {
                let payload = serde_json::to_string(&checked.message)?;
                let head: Option<(u64, String)> = tx
                    .query_row(
                        "SELECT seq,payload FROM batch_heads WHERE source=? AND epoch=?",
                        params![source, epoch],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()?;
                if let Some((seq, old)) = &head {
                    if batch_seq == seq && old == &payload {
                        return Ok(0);
                    }
                    if *batch_seq != seq + 1 {
                        bail!("stale, conflicting or out-of-order batch");
                    }
                } else if *batch_seq != 1 {
                    bail!("first batch sequence must be 1");
                }
                for event in events {
                    let payload = digest(&serde_json::to_string(event)?);
                    let old: Option<String> = tx.query_row("SELECT payload FROM event_ids WHERE source=? AND epoch=? AND event_id=?", params![source,epoch,event.id], |r| r.get(0)).optional()?;
                    if let Some(old) = old {
                        if old != payload {
                            bail!("event identity reused with different content");
                        }
                        continue;
                    }
                    tx.execute(
                        "INSERT INTO event_ids VALUES(?,?,?,?)",
                        params![source, epoch, event.id, payload],
                    )?;
                    tx.execute("INSERT INTO records(source,kind,object_id,occurred_at,data) VALUES(?,?,?,?,?)", params![source,event.kind,event.object_id,event.occurred_at,serde_json::to_string(&event.data)?])?;
                    // The evidence address and its projection work item are written in this same
                    // transaction as the material they point at: a batch that fails to commit
                    // leaves nothing behind, so the source can simply be replayed.
                    evidence::record_event(&tx,source,epoch,event,tx.last_insert_rowid(),&ingested_at,&ingested_at,&decoding)?;
                    inserted += 1;
                }
                coverage::record_attempt(&tx,source,epoch,source_configured(&disabled_items,source),true,&ingested_at)?;
                tx.execute("INSERT INTO checkpoints VALUES(?,?,?) ON CONFLICT(source,epoch) DO UPDATE SET value=excluded.value",params![source,epoch,checkpoint])?;
                tx.execute("INSERT INTO batch_heads VALUES(?,?,?,?) ON CONFLICT(source,epoch) DO UPDATE SET seq=excluded.seq,payload=excluded.payload",params![source,epoch,batch_seq,payload])?;
            }
            Message::Snapshot {
                source,
                scope,
                run_id,
                observed_at,
                complete,
                items,
            } => {
                if !complete {
                    bail!("incomplete snapshot: current state and history unchanged");
                }
                let payload = serde_json::to_string(&checked.message)?;
                let old_run: Option<String> = tx.query_row("SELECT digest FROM snapshot_receipts WHERE source=? AND scope=? AND run_id=?", params![source,scope,run_id], |r|r.get(0)).optional()?;
                let mut absent_records=Vec::new();
                let mut appeared_records=Vec::new();
                if let Some(old) = old_run {
                    if old != digest(&payload) {
                        bail!("snapshot run reused with different content");
                    }
                    return Ok(0);
                }
                let baseline: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM scopes WHERE source=? AND scope=?)",
                    params![source, scope],
                    |r| r.get(0),
                )?;
                let old: BTreeMap<String, String> = {
                    let mut q =
                        tx.prepare("SELECT id,data FROM objects WHERE source=? AND scope=?")?;
                    let rows =
                        q.query_map(params![source, scope], |r| Ok((r.get(0)?, r.get(1)?)))?;
                    rows.collect::<rusqlite::Result<_>>()?
                };
                let previous_observation: Option<String> = tx.query_row("SELECT json_extract(payload,'$.observed_at') FROM snapshots WHERE source=? AND scope=? ORDER BY rowid DESC LIMIT 1",params![source,scope],|r|r.get(0)).optional()?.flatten();
                let mut new: BTreeMap<String, String> = items
                    .iter()
                    .map(|i| Ok((i.id.clone(), serde_json::to_string(&i.data)?)))
                    .collect::<Result<_>>()?;
                for (id,data) in &old {if !disabled_items.is_empty() && disabled_items.contains(&item_controls::classify(source,id,"",&Value::Null)){new.insert(id.clone(),data.clone());}}
                for (id, data) in &new {
                    if disabled_items.contains(&item_controls::classify(source,id,"",&Value::Null)){continue;}
                    let before = old.get(id);
                    let unchanged = match before {
                        Some(previous) => pcscope_contracts::sensitive::equivalent(&serde_json::from_str(previous)?, &serde_json::from_str(data)?),
                        None => false,
                    };
                    let before_seen:Option<String>=if !unchanged&&before.is_some(){tx.query_row("SELECT seen_at FROM object_times WHERE source=? AND scope=? AND id=?",params![source,scope,id],|r|r.get(0)).optional()?.flatten()}else{None};
                    tx.execute("INSERT INTO object_times(source,scope,id,changed_at,seen_at) VALUES(?,?,?,?,?) ON CONFLICT(source,scope,id) DO UPDATE SET changed_at=CASE WHEN ? THEN excluded.changed_at ELSE object_times.changed_at END,seen_at=excluded.seen_at",
                        params![source,scope,id,if unchanged { None } else { Some(observed_at) },observed_at,!unchanged])?;
                    if unchanged {
                        continue;
                    }
                    let kind = if !baseline {
                        "baseline.observed"
                    } else if before.is_some() {
                        "object.changed"
                    } else {
                        "object.first_seen"
                    };
                    let mut detail = json!({"scope":scope,"before":before.map(|s|serde_json::from_str::<Value>(s)).transpose()?,"after":serde_json::from_str::<Value>(data)?,"basis":"snapshot","before_observed_at":before_seen.as_ref().or(previous_observation.as_ref())});
                    if kind=="object.first_seen"&&detail["after"].is_object(){detail["_snapshot_appearance_v1"]=json!(true);}
                    tx.execute("INSERT INTO records(source,kind,object_id,occurred_at,data) VALUES(?,?,?,?,?)",params![source,kind,id,observed_at,detail.to_string()])?;
                    if kind=="object.first_seen"&&detail["after"].is_object() {appeared_records.push((id.clone(),tx.last_insert_rowid()));}
                    inserted += 1;
                }
                for (id, data) in &old {
                    if new.contains_key(id) {
                        continue;
                    }
                    tx.execute(
                        "DELETE FROM object_times WHERE source=? AND scope=? AND id=?",
                        params![source, scope, id],
                    )?;
                    let detail = json!({"scope":scope,"before":serde_json::from_str::<Value>(data)?,"after":null,"basis":"snapshot","before_observed_at":previous_observation,"_snapshot_absence_v1":true});
                    tx.execute("INSERT INTO records(source,kind,object_id,occurred_at,data) VALUES(?,'object.no_longer_observed',?,?,?)",params![source,id,observed_at,detail.to_string()])?;
                    absent_records.push((id.clone(),tx.last_insert_rowid()));
                    inserted += 1;
                }
                tx.execute(
                    "DELETE FROM objects WHERE source=? AND scope=?",
                    params![source, scope],
                )?;
                for (id, data) in new {
                    tx.execute(
                        "INSERT INTO objects VALUES(?,?,?,?)",
                        params![source, scope, id, data],
                    )?;
                }
                store_snapshot(&tx, source, scope, run_id, &payload, unit_id, observed_at, &ingested_at, items)?;
                // Queue additions after their snapshot observations establish the object binding.
                // Initial baselines remain observations, rather than fabricated creation events.
                for (id,seq) in appeared_records {evidence::record_snapshot_appearance(&tx,source,scope,run_id,&id,seq,observed_at,&ingested_at)?;}
                // All present aliases/child records must project before missing records from this
                // same complete round; one disappearing registry role cannot hide its live object.
                for (id,seq) in absent_records {evidence::record_snapshot_absence(&tx,source,scope,run_id,&id,seq,observed_at,&ingested_at)?;}
                coverage::record_attempt(&tx,source,scope,source_configured(&disabled_items,source),true,&ingested_at)?;
            }
            Message::Gap {
                source,
                reason,
                observed_at,
            } => {
                let payload = serde_json::to_string(&checked.message)?;
                let old: Option<String> = tx
                    .query_row(
                        "SELECT payload FROM gaps WHERE source=? AND id=?",
                        params![source, checked.request_id],
                        |r| r.get(0),
                    )
                    .optional()?;
                if let Some(old) = old {
                    if old == payload {
                        return Ok(0);
                    }
                    bail!("gap identity reused with different content");
                }
                tx.execute(
                    "INSERT INTO gaps VALUES(?,?,?)",
                    params![source, checked.request_id, payload],
                )?;
                coverage::record_attempt(&tx,source,"",source_configured(&disabled_items,source),false,&ingested_at)?;
                inserted = coverage::gap(&tx,source,reason,observed_at)?;
            }
        }
        tx.execute(
            "UPDATE records SET unit_id=? WHERE seq>?",
            params![unit_id, previous_seq],
        )?;
        self_activity::ingest(&tx,previous_seq)?;
        tx.commit()?;
        // FULL-synchronous WAL commit already makes the accepted material durable. TRUNCATE
        // waits for history readers while holding the collection writer's mutex, delaying both
        // new material and model/display work by the busy timeout for every envelope. PASSIVE
        // copies the pages it can now; readers keep their snapshots and later commits reuse WAL.
        // Capacity enforcement retains its explicit reclamation/checkpoint path below.
        self.conn
            .execute_batch("PRAGMA wal_checkpoint(PASSIVE);")?;
        Ok(inserted)
    }

    /// Call only after every message from a successful collector invocation was committed.
    /// A successful shard must not clear failures from another shard of the same source.
    pub fn observe_collection_success(&mut self,messages:&[Envelope])->Result<usize> {
        let mut sources=std::collections::BTreeSet::new();
        let mut failed=std::collections::BTreeSet::new();
        for envelope in messages {
            match &envelope.message {
                Message::Gap{source,..}=>{failed.insert(source);},
                Message::Snapshot{source,complete:true,..}|Message::EventBatch{source,..}=>{sources.insert(source);},
                _=>{},
            }
        }
        let disabled=self.disabled_items()?;
        let tx=self.conn.transaction()?;
        let time:String=tx.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ','now')",[],|r|r.get(0))?;
        let previous_seq:i64=tx.query_row("SELECT coalesce(max(seq),0) FROM records",[],|r|r.get(0))?;
        let mut count=0;
        for source in sources.difference(&failed) {if !disabled.contains(&item_controls::classify(source,"","coverage.recovered",&Value::Null)){count+=coverage::recovered(&tx,source,&time)?;}}
        if count>0 {
            tx.execute("INSERT INTO retention_units(created_at) VALUES(?)",[&time])?;
            let unit=tx.last_insert_rowid();
            tx.execute("UPDATE records SET unit_id=? WHERE seq>?",params![unit,previous_seq])?;
        }
        tx.commit()?;
        Ok(count)
    }

    pub fn task_evidence(&self, seq: i64) -> Result<Value> {
        let (source,task,end,raw):(String,String,String,String)=self.conn.query_row("SELECT source,object_id,occurred_at,data FROM records WHERE seq=?",[seq],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
        if source!="pcscope.startup.task-details" && source!="pcscope.startup.tasks" {bail!("此记录不是计划任务快照");}
        let data:Value=serde_json::from_str(&raw)?;
        let mut start=data["before_observed_at"].as_str().map(str::to_owned);
        if start.is_none() {
            start=self.conn.query_row("SELECT json_extract(payload,'$.observed_at') FROM snapshots WHERE source=? AND scope=? AND julianday(json_extract(payload,'$.observed_at')) < julianday(?) ORDER BY julianday(json_extract(payload,'$.observed_at')) DESC LIMIT 1",params![source,data["scope"].as_str().unwrap_or(""),end],|r|r.get(0)).optional()?.flatten();
        }
        let Some(start)=start else {return Ok(json!({"status":"unknown_interval","events":[],"reason":"缺少前一次观察时间，无法可靠确定关联区间。首次基线也不能证明任务在此时被创建。"}));};
        let mut q=self.conn.prepare("SELECT seq,occurred_at,data FROM records WHERE (source='pcscope.eventlog.security' OR source='pcscope.eventlog.microsoft-windows-taskscheduler/operational') AND json_extract(data,'$.fields.TaskName') = ? COLLATE NOCASE AND julianday(occurred_at)>julianday(?) AND julianday(occurred_at)<=julianday(?) ORDER BY julianday(occurred_at),seq LIMIT 101")?;
        let rows=q.query_map(params![task,start,end],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?)))?;
        let mut events=Vec::new();
        for row in rows {let (id,time,text)=row?;events.push(json!({"seq":id,"occurred_at":time,"data":serde_json::from_str::<Value>(&text)?}));}
        let truncated=events.len()>100;events.truncate(100);
        Ok(json!({"status":if events.is_empty(){"no_evidence"}else{"candidates"},"from":start,"to":end,"task":task,"events":events,"truncated":truncated,"reason":"按同一任务完整路径和两次观察之间的事件时间匹配。日志证明相应操作发生，但不单独证明它导致某个快照字段变化；缺少日志不能证明没有修改。"}))
    }

    pub fn timeline(&self, before: Option<i64>, limit: usize) -> Result<Vec<Value>> {
        let mut q = self.conn.prepare("SELECT seq,source,kind,object_id,occurred_at,data FROM records WHERE seq < ? ORDER BY seq DESC LIMIT ?")?;
        let rows = q.query_map(
            params![before.unwrap_or(i64::MAX), limit.clamp(1, 1000)],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                ))
            },
        )?;
        let mut items=rows.map(|row| { let (seq,source,kind,object_id,occurred_at,data)=row?; Ok(json!({"seq":seq,"source":source,"kind":kind,"object_id":object_id,"occurred_at":occurred_at,"data":serde_json::from_str::<Value>(&data)?})) }).collect::<Result<Vec<_>>>()?;
        self.attach_activity(&mut items)?;
        Ok(items)
    }

    pub fn search_timeline(
        &self,
        query: &Value,
        before: Option<i64>,
        limit: usize,
    ) -> Result<Value> {
        let text = |key: &str| query[key].as_str().unwrap_or("");
        let search = text("search");
        if search.len() > 1024 {
            bail!("search is too long");
        }
        let mut filters=vec!["s.seq<?1".to_owned()];
        let mut values:Vec<rusqlite::types::Value>=vec![before.unwrap_or(i64::MAX).into()];
        for (key,column) in [("category","s.category"),("item","s.item"),("source","r.source"),("action","s.action"),("kind",if ["coverage","events"].contains(&text("kind")){"s.kind_group"}else{"s.kind"})] {
            if !text(key).is_empty(){values.push(text(key).to_owned().into());filters.push(format!("{column}=?{}",values.len()));}
        }
        match text("self_activity") {
            ""|"all"=>{},"hide"=>filters.push("NOT EXISTS(SELECT 1 FROM self_activity own WHERE own.seq=s.seq)".into()),
            "only"=>filters.push("s.seq IN (SELECT seq FROM self_activity)".into()),
            _=>bail!("invalid self activity filter"),
        }
        if !search.is_empty(){
            values.push(search.to_owned().into());
            filters.push(format!("instr(lower(r.object_id||' '||r.data||' '||r.source||' '||s.item_name),lower(?{}))>0",values.len()));
        }
        let limit=limit.clamp(1,100);values.push(((limit+1) as i64).into());
        let sql=format!("SELECT r.seq,r.source,r.kind,r.object_id,r.occurred_at,r.data,s.item,s.item_name,s.category,s.action FROM record_search s JOIN records r ON r.seq=s.seq WHERE {} ORDER BY s.seq DESC LIMIT ?{}",filters.join(" AND "),values.len());
        let tx=self.conn.unchecked_transaction()?;
        let mut stmt=tx.prepare(&sql)?;
        let rows=stmt.query_map(rusqlite::params_from_iter(values),|r|Ok(json!({"seq":r.get::<_,i64>(0)?,"source":r.get::<_,String>(1)?,"kind":r.get::<_,String>(2)?,"object_id":r.get::<_,String>(3)?,"occurred_at":r.get::<_,String>(4)?,"data":r.get::<_,String>(5)?,"item":r.get::<_,String>(6)?,"item_name":r.get::<_,String>(7)?,"category":r.get::<_,String>(8)?,"action":r.get::<_,String>(9)?})))?;
        let mut items=Vec::new();
        for row in rows {let mut row=row?;row["data"]=serde_json::from_str(row["data"].as_str().unwrap())?;items.push(row);}
        let more=items.len()>limit;items.truncate(limit);drop(stmt);
        // Covering index scan: no evidence payload reads and no reclassification of historical rows.
        let mut facets=tx.prepare("SELECT DISTINCT item,item_name,category FROM record_search ORDER BY category,item_name")?;
        let sources=facets.query_map([],|r|Ok(json!({"item":r.get::<_,String>(0)?,"item_name":r.get::<_,String>(1)?,"category":r.get::<_,String>(2)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;
        drop(facets);tx.commit()?;
        let mut action_filters=vec!["1=1".to_owned()];let mut action_values=Vec::new();
        for (key,column) in [("category","s.category"),("item","s.item"),("source","r.source")] {
            if !text(key).is_empty(){action_values.push(text(key));action_filters.push(format!("{column}=?{}",action_values.len()));}
        }
        let action_join=if text("source").is_empty(){""}else{" JOIN records r ON r.seq=s.seq"};
        let action_sql=format!("SELECT DISTINCT s.action FROM record_search s{action_join} WHERE {} ORDER BY s.action",action_filters.join(" AND "));
        let actions:Vec<String>=self.conn.prepare(&action_sql)?.query_map(rusqlite::params_from_iter(action_values),|r|r.get(0))?.collect::<rusqlite::Result<_>>()?;
        self.attach_activity(&mut items)?;
        Ok(json!({"items":items,"more":more,"sources":sources,"actions":actions}))
    }
    pub fn checkpoint(&self, source: &str, epoch: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT value FROM checkpoints WHERE source=? AND epoch=?",
                params![source, epoch],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// The persisted material adapters are handed, in the order it arrived.
    ///
    /// `after_seq` resumes where a previous read stopped, so a projection can work through evidence
    /// in bounded batches instead of loading the table. This is the store-backed form of the
    /// fixture-line reader in `pcscope_contracts::reference`; both produce the same `EvidenceInput`.
    pub fn evidence_inputs(&self, after_seq: i64, limit: usize) -> Result<Vec<EvidenceInput>> {
        evidence::inputs(&self.conn, after_seq, limit)
    }

    /// How much projection work is outstanding, by state.
    pub fn evidence_queue(&self, projection: &str) -> Result<Value> {
        evidence::queue_summary(&self.conn, projection)
    }

    /// Resolves one candidate against the stored instances and commits what it decided.
    pub fn apply_identity(&self, request: &pcscope_contracts::identity::IdentityRequest) -> Result<identity::Outcome> {
        let tx = self.conn.unchecked_transaction()?;
        let outcome = identity::apply(&tx, request)?;
        tx.commit()?;
        Ok(outcome)
    }

    /// Concludes absence for instances a complete observation of a scope did not cover.
    pub fn conclude_absence(
        &self,
        source: &str,
        scope: &str,
        hints: &pcscope_contracts::identity::LifecycleHints,
        observed_at: &str,
    ) -> Result<Vec<(u64, pcscope_contracts::identity::LifecycleTransition)>> {
        let tx = self.conn.unchecked_transaction()?;
        let decided = identity::conclude_absence(&tx, source, scope, hints, observed_at)?;
        tx.commit()?;
        Ok(decided)
    }

    /// The stored summary of one instance.
    pub fn instance_summary(&self, instance_id: u64) -> Result<Option<Value>> {
        identity::summary(&self.conn, instance_id)
    }

    /// What is known about every source and scope: whether it is switched on, how its last attempt
    /// went, when it last succeeded, whether the retained history has a break in it, and how much
    /// projection work is outstanding or has failed.
    ///
    /// Collection failures and parsing failures are reported separately because they call for
    /// different responses. A collection failure means the material never arrived — permissions,
    /// the source being unavailable, a gap the source declared. A parsing failure means it arrived
    /// and nothing could be made of it — no adapter yet, or candidates the contract rejected. A
    /// source can be perfectly collectable and completely unparsed.
    pub fn coverage_report(&self) -> Result<Value> {
        Ok(Value::Array(coverage::report(&self.conn, None)?))
    }

    /// S22: the query layer, reading the object model rather than the raw store.
    pub fn query(&self, request: &pcscope_contracts::query::QueryRequest) -> Result<pcscope_contracts::query::QueryResponse> {
        if !self.conn.is_autocommit(){return query::answer(&self.conn,request);}
        let tx=self.conn.unchecked_transaction()?;
        let response=query::answer(&tx,request)?;
        tx.commit()?;
        Ok(response)
    }

    /// The migration needs the connection to write in batches rather than through the store's own
    /// handle, and the CLI needs the same connection for a read-only preflight.
    pub fn reader(&self) -> &Connection {
        &self.conn
    }

    /// S28: reconcile the model against the material it was built from.
    pub fn reconcile(&self) -> Result<Value> {
        reconcile::report(&self.conn)
    }

    /// Stable collection-installation identity, preserved with database backups.
    pub fn projection_installation(&self, machine: &str) -> Result<String> {
        anyhow::ensure!(!machine.trim().is_empty(), "projection machine identity is empty");
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("INSERT OR IGNORE INTO settings(key,value) VALUES('projection_machine',?)", [machine])?;
        let saved: String = tx.query_row("SELECT value FROM settings WHERE key='projection_machine'", [], |r| r.get(0))?;
        anyhow::ensure!(saved == machine, "projection database belongs to another machine; explicit migration required");
        let bound: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM settings WHERE key='projection_installation')", [], |r|r.get(0))?;
        let existing: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM object_instances)", [], |r|r.get(0))?;
        anyhow::ensure!(bound || !existing, "existing model has no runtime identity binding; explicit reconciliation required");
        tx.execute_batch("INSERT OR IGNORE INTO settings(key,value) VALUES('projection_installation',lower(hex(randomblob(16))))")?;
        let installation = tx.query_row("SELECT value FROM settings WHERE key='projection_installation'", [], |r| r.get(0))?;
        tx.commit()?;
        Ok(installation)
    }

    /// Cheap committed change token for page refresh; no raw history scan or writer lock.
    pub fn read_model_revision(&self) -> Result<String> {
        let action:i64=self.conn.query_row("SELECT coalesce(max(action_row),0) FROM object_actions",[],|r|r.get(0))?;
        let (generation,published):(i64,i64)=if display::available(&self.conn)? {
            self.conn.query_row("SELECT generation,published FROM display_build WHERE id=1",[],|r|Ok((r.get(0)?,r.get(1)?)))?
        }else{(0,0)};
        Ok(format!("{action}:{generation}:{published}"))
    }

    /// Drains pending projection work into facts, history and actions.
    pub fn project_pending(&self, machine_id: &str, installation_id: &str, limit: usize) -> Result<projection::Report> {
        let tx = self.conn.unchecked_transaction()?;
        let report = projection::project_pending(&tx, machine_id, installation_id, limit)?;
        tx.commit()?;
        Ok(report)
    }
    /// Existing projection worker's bounded display maintenance; never called by a UI reader.
    pub fn prepare_display(&self, limit: usize, budget: std::time::Duration) -> Result<Value> {
        let tx=self.conn.unchecked_transaction()?;
        let report=display::advance(&tx,limit,budget)?;
        tx.commit()?;
        Ok(report)
    }
    /// Explicit resumable compatibility repair. The caller controls each bounded batch.
    pub fn repair_snapshot_absence(&self,limit:usize)->Result<Value> {
        let tx=self.conn.unchecked_transaction()?;let report=evidence::repair_snapshot_absence(&tx,limit)?;tx.commit()?;Ok(report)
    }
    /// Explicit bounded recovery of retained additions; never edits the original records.
    pub fn repair_snapshot_appearance(&self,limit:usize)->Result<Value> {
        let tx=self.conn.unchecked_transaction()?;let report=evidence::repair_snapshot_appearance(&tx,limit)?;tx.commit()?;Ok(report)
    }

    pub fn objects(&self, offset: usize, limit: usize) -> Result<Vec<Value>> {
        let mut q = self.conn.prepare(
            "SELECT source,scope,id,data FROM objects ORDER BY source,scope,id LIMIT ? OFFSET ?",
        )?;
        let rows = q.query_map(params![limit.clamp(1, 500), offset], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?;
        rows.map(|r|{let(source,scope,id,data)=r?;Ok(json!({"source":source,"scope":scope,"id":id,"data":serde_json::from_str::<Value>(&data)?}))}).collect()
    }

    /// Filter the entire current inventory before counting and paging. SQL values are bound;
    /// the only dynamic SQL is an allowlisted ordering expression.
    pub fn search_objects(&self, query: &Value, offset: usize, limit: usize) -> Result<Value> {
        let field = |key: &str| query[key].as_str().unwrap_or("");
        let search = field("search");
        if search.len() > 1024 {
            bail!("search is too long");
        }
        let category = field("category");
        let source = field("source");
        let item = field("item");
        let since = field("since");
        if !since.is_empty() {
            let valid: bool =
                self.conn
                    .query_row("SELECT julianday(?) IS NOT NULL", [since], |r| r.get(0))?;
            if !valid {
                bail!("invalid change time");
            }
        }
        let order = match field("sort") {
            "" | "name_asc" => "name COLLATE NOCASE ASC",
            "name_desc" => "name COLLATE NOCASE DESC",
            "changed_desc" => "julianday(changed_at) DESC",
            "seen_desc" => "julianday(seen_at) DESC",
            "source_asc" => "source ASC",
            "item_asc" => "item_name ASC",
            _ => bail!("unsupported object sort"),
        };
        let item_sql = collection::item("o.source", "o.id");
        let item_name = collection::field("item", "name", "source");
        let base = format!("WITH classified AS (SELECT o.*,t.changed_at,t.seen_at,{item_sql} AS item,
            CAST(COALESCE(json_extract(o.data,'$.DisplayName'),json_extract(o.data,'$.display_name'),json_extract(o.data,'$.Name'),json_extract(o.data,'$.name'),o.id) AS TEXT) AS name,
            CASE WHEN o.source='pcscope.startup.services' THEN 'services'
                 WHEN o.source LIKE '%driver%' THEN 'drivers'
                 WHEN o.source='pcscope.software' THEN 'software'
                 WHEN o.source LIKE 'pcscope.%' AND instr(substr(o.source,9),'.')>0
                 THEN substr(o.source,9,instr(substr(o.source,9),'.')-1)
                 ELSE 'other' END AS category
            FROM objects o LEFT JOIN object_times t USING(source,scope,id)), inventory AS (SELECT classified.*,{item_name} AS item_name FROM classified) ");
        let filter =
            " WHERE (?1='' OR instr(lower(id||' '||data||' '||source||' '||scope||' '||item_name),lower(?1))>0)
            AND (?2='' OR category=?2) AND (?3='' OR source=?3)
            AND (?4='' OR julianday(changed_at)>=julianday(?4)) AND (?5='' OR item=?5)";
        // Keep count and page consistent even if another connection commits a collection.
        let tx = self.conn.unchecked_transaction()?;
        let total: i64 = tx.query_row(
            &format!("{base} SELECT count(*) FROM inventory{filter}"),
            params![search, category, source, since, item],
            |r| r.get(0),
        )?;
        let limit = limit.clamp(1, 100);
        let offset = offset.min(((total - 1).max(0) as usize / limit) * limit);
        let mut statement = tx.prepare(&format!("{base} SELECT source,scope,id,data,changed_at,seen_at,category,name,item,item_name FROM inventory{filter} ORDER BY {order},source,scope,id LIMIT ?6 OFFSET ?7"))?;
        let rows = statement.query_map(params![search,category,source,since,item,limit,offset], |r| {
            Ok(json!({"source":r.get::<_,String>(0)?,"scope":r.get::<_,String>(1)?,"id":r.get::<_,String>(2)?,
                "data":r.get::<_,String>(3)?,"changed_at":r.get::<_,Option<String>>(4)?,"seen_at":r.get::<_,Option<String>>(5)?,
                "category":r.get::<_,String>(6)?,"name":r.get::<_,String>(7)?,"item":r.get::<_,String>(8)?,"item_name":r.get::<_,String>(9)?}))
        })?;
        let mut items = Vec::new();
        for row in rows {
            let mut row = row?;
            row["data"] = serde_json::from_str(row["data"].as_str().unwrap())?;
            items.push(row);
        }
        let mut facets = tx.prepare(&format!(
            "{base} SELECT DISTINCT category,source,item,item_name FROM inventory ORDER BY category,item_name,source"
        ))?;
        let sources = facets
            .query_map([], |r| {
                Ok(json!({"category":r.get::<_,String>(0)?,"source":r.get::<_,String>(1)?,"item":r.get::<_,String>(2)?,"item_name":r.get::<_,String>(3)?}))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(facets);
        drop(statement);
        tx.commit()?;
        Ok(json!({"items":items,"total":total,"offset":offset,"limit":limit,"sources":sources}))
    }

    pub fn snapshots(&self, limit: usize) -> Result<Vec<Value>> {
        let mut q=self.conn.prepare("SELECT source,scope,run_id,json_extract(payload,'$.observed_at') FROM snapshots ORDER BY rowid DESC LIMIT ?")?;
        let rows=q.query_map([limit.clamp(1,200)],|r|Ok(json!({"source":r.get::<_,String>(0)?,"scope":r.get::<_,String>(1)?,"run_id":r.get::<_,String>(2)?,"observed_at":r.get::<_,String>(3)?})))?;
        rows.map(|r| Ok(r?)).collect()
    }

    pub fn compare(
        &self,
        source: &str,
        scope: &str,
        left: &str,
        right: &str,
    ) -> Result<Vec<Value>> {
        let read = |run: &str| -> Result<BTreeMap<String, Value>> {
            let payload: String = self
                .conn
                .query_row(
                    "SELECT payload FROM snapshots WHERE source=? AND scope=? AND run_id=?",
                    params![source, scope, run],
                    |r| r.get(0),
                )
                .context("snapshot unavailable in retained history")?;
            match serde_json::from_str::<Message>(&payload)? {
                Message::Snapshot { items, .. } => {
                    Ok(items.into_iter().map(|i| (i.id, i.data)).collect())
                }
                _ => bail!("invalid snapshot payload"),
            }
        };
        let before = read(left)?;
        let after = read(right)?;
        let ids: std::collections::BTreeSet<_> = before.keys().chain(after.keys()).collect();
        Ok(ids
            .into_iter()
            .filter(|id| match (before.get(*id),after.get(*id)) {
                (Some(a),Some(b)) => !pcscope_contracts::sensitive::equivalent(a,b),
                (None,None) => false,
                _ => true,
            })
            .map(|id| json!({"id":id,"before":before.get(id),"after":after.get(id)}))
            .collect())
    }

    pub fn counts(&self) -> Result<Value> {
        let count = |table: &str| -> Result<i64> {
            Ok(self
                .conn
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))?)
        };
        Ok(
            json!({"records":count("records")?,"objects":count("objects")?,"snapshots":count("snapshots")?}),
        )
    }

    pub fn source_resume(&self, source: &str) -> Result<Option<Value>> {
        Ok(self.conn.query_row("SELECT c.epoch,c.value,b.seq FROM checkpoints c JOIN batch_heads b ON c.source=b.source AND c.epoch=b.epoch WHERE c.source=? ORDER BY c.rowid DESC LIMIT 1",[source],|r|Ok(json!({"epoch":r.get::<_,String>(0)?,"checkpoint":r.get::<_,String>(1)?,"batch_seq":r.get::<_,u64>(2)?}))).optional()?)
    }

    pub fn internal_event(&mut self, kind: &str, time: &str, data: Value) -> Result<()> {
        let seq: u64 = self
            .conn
            .query_row(
                "SELECT seq FROM batch_heads WHERE source='pcscope.core' AND epoch='core-v1'",
                [],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(0)
            + 1;
        let envelope = Envelope {
            protocol_version: 1,
            request_id: format!("core-{seq}"),
            message: Message::EventBatch {
                source: "pcscope.core".into(),
                epoch: "core-v1".into(),
                batch_seq: seq,
                checkpoint: seq.to_string(),
                events: vec![pcscope_contracts::Event {
                    id: seq.to_string(),
                    occurred_at: time.into(),
                    kind: kind.into(),
                    object_id: "pcscope.service".into(),
                    data,
                }],
            },
        };
        // Recovery must be able to start and report failures even above the queue guard.
        // Only this in-process constructor gets the exception; a collector claiming the
        // pcscope.core source still passes the normal ingest gate. Capacity limits remain.
        self.ingest_unchecked(&envelope, true).map_err(explain_full_store)?;
        Ok(())
    }

    pub fn begin_session(&mut self, time: &str) -> Result<()> {
        let previous: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM settings WHERE key='clean_shutdown'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        if previous.as_deref() == Some("false") {
            let heartbeat: Option<String> = self
                .conn
                .query_row(
                    "SELECT value FROM settings WHERE key='last_heartbeat'",
                    [],
                    |r| r.get(0),
                )
                .optional()?;
            self.internal_event("coverage.unclean_shutdown",time,json!({"last_heartbeat":heartbeat,"basis":"missing clean shutdown marker; exact gap and recoverable source data unknown"}))?;
        }
        self.internal_event(
            "collector.started",
            time,
            json!({"mode":"development-user-daemon"}),
        )?;
        self.conn.execute("INSERT INTO settings VALUES('clean_shutdown','false') ON CONFLICT(key) DO UPDATE SET value='false'",[])?;
        self.heartbeat(time)
    }

    pub fn heartbeat(&self, time: &str) -> Result<()> {
        self.conn.execute("INSERT INTO settings VALUES('last_heartbeat',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[time])?;
        Ok(())
    }

    pub fn end_session(&mut self, time: &str) -> Result<()> {
        self.internal_event("collector.stopped", time, json!({"clean":true}))?;
        self.conn.execute(
            "UPDATE settings SET value='true' WHERE key='clean_shutdown'",
            [],
        )?;
        Ok(())
    }

    pub fn plugin_settings(&self) -> Result<Value> {
        let value: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM settings WHERE key='plugin_schedule'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        Ok(value
            .map(|v| serde_json::from_str(&v))
            .transpose()?
            .unwrap_or(json!({})))
    }

    pub fn file_watch_settings(&self) -> Result<Value> {
        let text: Option<String> = self.conn.query_row("SELECT value FROM settings WHERE key='file_watch'", [], |r| r.get(0)).optional()?;
        match text {Some(text)=>Ok(serde_json::from_str(&text)?),None=>Ok(json!({"files":[],"revision":0}))}
    }
    pub fn directory_watch_settings(&self) -> Result<Value> {
        let text: Option<String> = self.conn.query_row("SELECT value FROM settings WHERE key='directory_watch'", [], |row| row.get(0)).optional()?;
        Ok(match text { Some(text) => serde_json::from_str(&text)?, None => json!({"directories":[],"revision":0}) })
    }
    pub fn set_directory_watch_settings(&mut self, directories: Vec<String>, time: &str) -> Result<Value> {
        anyhow::ensure!(directories.len() <= 100, "最多添加 100 个目录");
        let mut normalized=Vec::new(); let mut seen=std::collections::HashSet::new();
        for directory in directories {
            let directory=directory.trim().replace('/',"\\"); let bytes=directory.as_bytes();
            anyhow::ensure!(bytes.len() >= 4 && bytes[0].is_ascii_alphabetic() && &bytes[1..3] == b":\\" && directory.len() <= 512, "请输入本机目录完整路径");
            anyhow::ensure!(!directory[3..].chars().any(|c| c < ' ' || "*?<>|\":".contains(c)), "目录路径不支持通配符或设备路径");
            anyhow::ensure!(directory[3..].split('\\').all(|part| !part.is_empty() && part != "." && part != ".." && !part.ends_with([' ','.'])), "目录路径不能包含相对段或空段");
            anyhow::ensure!(!std::path::Path::new(&directory).is_file(), "该路径是文件，请使用文件名单");
            if seen.insert(directory.to_lowercase()) { normalized.push(directory); }
        }
        let previous=self.directory_watch_settings()?;
        if previous["directories"] == json!(normalized) { return Ok(previous) }
        let value=json!({"directories":normalized,"revision":previous["revision"].as_u64().unwrap_or(0).checked_add(1).context("directory revision overflow")?});
        let tx=self.conn.transaction()?;
        tx.execute("INSERT INTO settings VALUES('directory_watch',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [value.to_string()])?;
        tx.commit()?;
        self.internal_event("collector.directory_watch_settings", time, value.clone())?;
        Ok(value)
    }
    pub fn set_file_watch_settings(&mut self, files: Vec<String>, time: &str) -> Result<Value> {
        anyhow::ensure!(files.len()<=100,"最多添加 100 个文件");
        let mut normalized=Vec::new();let mut seen=std::collections::HashSet::new();
        for file in files {
            let file=file.trim().replace('/',"\\");let bytes=file.as_bytes();
            anyhow::ensure!(bytes.len()>=4 && bytes[0].is_ascii_alphabetic() && &bytes[1..3]==b":\\" && file.len()<=512,"请输入本机文件的完整路径，例如 C:\\Users\\name\\file.txt");
            anyhow::ensure!(!file[3..].chars().any(|c|c<' '||"*?<>|\":".contains(c)),"文件路径不支持通配符、设备路径或备用数据流");
            anyhow::ensure!(file[3..].split('\\').all(|part| !part.is_empty() && part!="." && part!=".." && !part.ends_with([' ','.'])),"路径不能包含空段、相对段、尾随空格或句点");
            anyhow::ensure!(!std::path::Path::new(&file).is_dir(),"请添加具体文件，不接受目录：{file}");
            if seen.insert(file.to_lowercase()) {normalized.push(file);}
        }
        let prior=self.file_watch_settings()?;
        if prior["files"]==json!(normalized) {return Ok(prior);}
        let value=json!({"files":normalized,"revision":prior["revision"].as_u64().unwrap_or(0).checked_add(1).context("watch revision overflow")?});
        self.conn.execute("INSERT INTO settings VALUES('file_watch',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[value.to_string()])?;
        self.internal_event("collector.file_watch_settings",time,value.clone())?;
        Ok(value)
    }
    pub fn monitoring_settings(&self) -> Result<Value> {
        let text: Option<String> = self.conn.query_row("SELECT value FROM settings WHERE key='monitoring'", [], |r| r.get(0)).optional()?;
        match text { Some(text) => Ok(serde_json::from_str(&text)?), None => Ok(json!({"mode":"low","repair":false,"configured":false})) }
    }
    pub fn runtime_watch_settings(&self) -> Result<Value> {
        let text:Option<String>=self.conn.query_row("SELECT value FROM settings WHERE key='runtime_watch'",[],|r|r.get(0)).optional()?;
        Ok(text.map(|s|serde_json::from_str(&s)).transpose()?.unwrap_or(json!({"enabled":true,"module_process_names":["sihost"],"revision":0})))
    }
    pub fn set_runtime_watch_settings(&mut self,enabled:bool,names:Vec<String>,time:&str)->Result<Value> {
        anyhow::ensure!(names.len()<=32,"最多 32 个精确进程名称");
        let mut normalized=BTreeSet::new();
        for name in names {let name=name.trim().to_ascii_lowercase();let name=name.strip_suffix(".exe").unwrap_or(&name).to_owned();
            anyhow::ensure!(!name.is_empty()&&name.len()<=100&&name.bytes().all(|c|c.is_ascii_alphanumeric()||b"_.-".contains(&c)),"仅支持精确进程名，不支持路径、通配符或命令");normalized.insert(name);}
        let old=self.runtime_watch_settings()?;let names:Vec<_>=normalized.into_iter().collect();
        if old["enabled"]==enabled && old["module_process_names"]==json!(names) {return Ok(old);}
        let value=json!({"enabled":enabled,"module_process_names":names,"revision":old["revision"].as_u64().unwrap_or(0)+1});
        let tx=self.conn.transaction()?;
        tx.execute("INSERT INTO settings VALUES('runtime_watch',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[value.to_string()])?;
        tx.commit()?;self.internal_event("collector.runtime_watch_settings",time,value.clone())?;
        Ok(value)
    }
    pub fn runtime_watch_state(&self)->Result<Value> {
        let text:Option<String>=self.conn.query_row("SELECT value FROM settings WHERE key='runtime_watch_state'",[],|r|r.get(0)).optional()?;
        Ok(text.map(|s|serde_json::from_str(&s)).transpose()?.unwrap_or(json!({})))
    }
    pub fn save_runtime_watch_state(&self,value:&Value)->Result<()> {
        let text=serde_json::to_string(value)?;anyhow::ensure!(text.len()<=32*1024*1024,"Runtime watch state exceeds bounded storage budget");
        self.conn.execute("INSERT INTO settings VALUES('runtime_watch_state',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[text])?;Ok(())
    }
    pub fn set_monitoring_settings(&mut self, value: &Value, time: &str) -> Result<()> {
        self.conn.execute("INSERT INTO settings VALUES('monitoring',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[value.to_string()])?;
        self.internal_event("collector.monitoring_settings", time, value.clone())?;
        Ok(())
    }
    pub fn show_sensitive(&self) -> Result<bool> {
        let value: Option<String> = self.conn.query_row("SELECT value FROM settings WHERE key='show_sensitive'", [], |r| r.get(0)).optional()?;
        Ok(value.as_deref() == Some("true"))
    }
    pub fn set_show_sensitive(&self, show: bool) -> Result<()> {
        self.conn.execute("INSERT INTO settings(key,value) VALUES('show_sensitive',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [if show {"true"} else {"false"}])?;
        Ok(())
    }
    pub fn save_plugin_settings(&self, value: &Value) -> Result<()> {
        self.conn.execute("INSERT INTO settings VALUES('plugin_schedule',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[value.to_string()])?;
        Ok(())
    }
    fn setting(&self, key: &str) -> Result<String> {
        Ok(self
            .conn
            .query_row("SELECT value FROM settings WHERE key=?", [key], |r| {
                r.get(0)
            })?)
    }

    // Reject invalid/replayed requests before retention can delete history.
    fn preflight(&self, envelope: &Envelope) -> Result<bool> {
        let payload = serde_json::to_string(&envelope.message)?;
        match &envelope.message {
            Message::Snapshot {
                source,
                scope,
                run_id,
                ..
            } => {
                let old:Option<String>=self.conn.query_row("SELECT digest FROM snapshot_receipts WHERE source=? AND scope=? AND run_id=?",params![source,scope,run_id],|r|r.get(0)).optional()?;
                if let Some(old) = old {
                    if old != digest(&payload) {
                        bail!("snapshot identity conflict");
                    }
                    return Ok(true);
                }
            }
            Message::EventBatch {
                source,
                epoch,
                batch_seq,
                events,
                ..
            } => {
                let old: Option<(u64, String)> = self
                    .conn
                    .query_row(
                        "SELECT seq,payload FROM batch_heads WHERE source=? AND epoch=?",
                        params![source, epoch],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()?;
                if let Some((seq, old)) = old {
                    if seq == *batch_seq && old == payload {
                        return Ok(true);
                    }
                    if *batch_seq != seq + 1 {
                        bail!("stale, conflicting or out-of-order batch");
                    }
                } else if *batch_seq != 1 {
                    bail!("first batch sequence must be 1");
                }
                for event in events {
                    let old:Option<String>=self.conn.query_row("SELECT payload FROM event_ids WHERE source=? AND epoch=? AND event_id=?",params![source,epoch,event.id],|r|r.get(0)).optional()?;
                    if let Some(old) = old {
                        if old != digest(&serde_json::to_string(event)?) {
                            bail!("event identity conflict");
                        }
                    }
                }
            }
            Message::Gap { source, .. } => {
                let old: Option<String> = self
                    .conn
                    .query_row(
                        "SELECT payload FROM gaps WHERE source=? AND id=?",
                        params![source, envelope.request_id],
                        |r| r.get(0),
                    )
                    .optional()?;
                if let Some(old) = old {
                    if old != payload {
                        bail!("gap identity conflict");
                    }
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    fn live_bytes(&self) -> Result<u64> {
        let pages: u64 = self.conn.query_row("PRAGMA page_count", [], |r| r.get(0))?;
        let free: u64 = self
            .conn
            .query_row("PRAGMA freelist_count", [], |r| r.get(0))?;
        let size: u64 = self.conn.query_row("PRAGMA page_size", [], |r| r.get(0))?;
        Ok(pages.saturating_sub(free) * size)
    }

    fn physical_bytes(&self) -> Result<u64> {
        let Some(path) = &self.path else {
            return self.live_bytes();
        };
        let mut bytes = 0;
        for suffix in ["", "-wal", "-shm"] {
            let mut name = path.as_os_str().to_os_string();
            name.push(suffix);
            match std::fs::metadata(PathBuf::from(name)) {
                Ok(m) => bytes += m.len(),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(bytes)
    }

    pub fn retention(&self) -> Result<Value> {
        let earliest: Option<String> = self
            .conn
            .query_row(
                "SELECT occurred_at FROM records ORDER BY seq LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()?;
        Ok(
            json!({"mode":self.setting("retention_mode")?,"quota_bytes":self.setting("quota_bytes")?.parse::<u64>().unwrap_or(DEFAULT_QUOTA),"physical_bytes":self.physical_bytes()?,"live_bytes":self.live_bytes()?,"pruned_units":self.setting("pruned_units")?.parse::<u64>()?,"earliest_retained_observation":earliest,"budget_policy":"one-third database budget; remainder reserved for WAL and maintenance","minimum_quota_bytes":MIN_QUOTA}),
        )
    }

    pub fn set_retention(&mut self, mode: &str, quota: u64) -> Result<Value> {
        if mode != "rolling" && mode != "unlimited" {
            bail!("invalid retention mode");
        }
        if !(MIN_QUOTA..=16_000_000_000_000).contains(&quota) {
            bail!("quota must be between 32 MiB and 16 TB");
        }
        let tx = self.conn.transaction()?;
        tx.execute(
            "UPDATE settings SET value=? WHERE key='retention_mode'",
            [mode],
        )?;
        tx.execute(
            "UPDATE settings SET value=? WHERE key='quota_bytes'",
            [quota.to_string()],
        )?;
        tx.commit()?;
        self.maintain_capacity(0)?;
        self.retention()
    }

    /// Reclaim oldest committed history units. Current objects and dedup receipts
    /// remain; if required metadata no longer fits, stop accepting data explicitly.
    fn maintain_capacity(&mut self, reserve: u64) -> Result<()> {
        if self.setting("retention_mode")? == "unlimited" {
            self.conn
                .execute_batch("PRAGMA max_page_count=4294967294;")?;
            return Ok(());
        }
        let quota = self.setting("quota_bytes")?.parse::<u64>()?;
        let budget = quota.saturating_sub(1048576) / 3;
        if reserve > budget {
            bail!("batch requires more than configured database budget");
        }
        while self.live_bytes()?.saturating_add(reserve) > budget {
            let unit: Option<i64> = self
                .conn
                .query_row(
                    "SELECT id FROM retention_units ORDER BY id LIMIT 1",
                    [],
                    |r| r.get(0),
                )
                .optional()?;
            let Some(unit) = unit else {
                bail!("required current state and dedup metadata exceed quota; increase quota or select unlimited");
            };
            let tx = self.conn.transaction()?;
            // Mark references before the evidence goes: a fact or action whose evidence is pruned
            // keeps its own value and observation time but must not present a dangling pointer.
            let pruned_at: String = tx.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ','now')", [], |r| r.get(0))?;
            model::mark_pruned_records(&tx, unit, &pruned_at)?;
            model::mark_pruned_snapshots(&tx, unit, &pruned_at)?;
            // The evidence index and the references into it are marked before the material goes, so
            // nothing is left pointing at a row that is not there.
            evidence::mark_pruned_for_unit(&tx, unit, &pruned_at)?;
            tx.execute("DELETE FROM records WHERE unit_id=?", [unit])?;
            tx.execute("DELETE FROM snapshots WHERE unit_id=?", [unit])?;
            tx.execute("DELETE FROM retention_units WHERE id=?", [unit])?;
            // Tags retain their own provenance; discard expired lookup metadata only
            // while rolling retention actually removes history. Unlimited keeps it.
            tx.execute("DELETE FROM own_processes WHERE seen_ms < (SELECT CAST(round((julianday(created_at)-2440587.5)*86400000) AS INTEGER) FROM retention_units ORDER BY id LIMIT 1)",[])?;
            tx.execute(
                "UPDATE settings SET value=CAST(value AS INTEGER)+1 WHERE key='pruned_units'",
                [],
            )?;
            tx.execute("INSERT INTO settings VALUES('last_pruned_unit',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[unit.to_string()])?;
            tx.commit()?;
        }
        self.conn
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        loop {
            let free: u64 = self
                .conn
                .query_row("PRAGMA freelist_count", [], |r| r.get(0))?;
            if free == 0 {
                break;
            }
            self.reclaim_page_batch()?;
            self.conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
            let remaining: u64 = self
                .conn
                .query_row("PRAGMA freelist_count", [], |r| r.get(0))?;
            if remaining >= free {
                bail!("database free pages cannot be reclaimed");
            }
        }
        let page_size: u64 = self.conn.query_row("PRAGMA page_size", [], |r| r.get(0))?;
        self.conn
            .execute_batch(&format!("PRAGMA max_page_count={};", budget / page_size))?;
        if self.physical_bytes()? > quota {
            bail!("physical storage remains above configured quota");
        }
        Ok(())
    }

    fn reclaim_page_batch(&self)->Result<()> {
        // incremental_vacuum yields once per reclaimed page. execute_batch only steps once,
        // which turned a 256-page batch into one page followed by a disk checkpoint.
        let mut statement=self.conn.prepare("PRAGMA incremental_vacuum(256)")?;
        let mut rows=statement.query([])?;
        while rows.next()?.is_some() {}
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn committed_collection_does_not_wait_for_an_active_history_reader() {
        let directory=std::env::temp_dir().join(format!("pcscope-reader-writer-{}-{}",std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&directory).unwrap();let path=directory.join("history.db");
        let elapsed={
            let mut store=Store::open(&path).unwrap();
            store.ingest(&snapshot("reader-before",true,json!([{"id":"a","data":{"value":1}}]))).unwrap();
            let reader=Connection::open_with_flags(&path,rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
            reader.execute_batch("BEGIN;").unwrap();
            let count=|conn:&Connection|conn.query_row("SELECT count(*) FROM snapshots",[],|row|row.get::<_,i64>(0)).unwrap();
            assert_eq!(count(&reader),1); // Actually acquire a WAL read snapshot before writing.
            store.conn.busy_timeout(std::time::Duration::from_secs(2)).unwrap();
            let mut timings=Vec::new();
            for index in 2..=101 {
                let next=snapshot(&format!("reader-{index}"),true,json!([{"id":"a","data":{"value":index}}]));
                let started=std::time::Instant::now();assert_eq!(store.ingest(&next).unwrap(),1);timings.push(started.elapsed());
                assert_eq!(store.ingest(&next).unwrap(),0,"replaying acknowledged material stays idempotent");
            }
            assert_eq!(count(&reader),1,"the existing history reader retains a consistent snapshot");
            let fresh=Connection::open_with_flags(&path,rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
            assert_eq!(count(&fresh),101,"a fresh reader sees all committed collections immediately");
            assert_eq!(store.conn.query_row("PRAGMA synchronous",[],|row|row.get::<_,i64>(0)).unwrap(),2);
            reader.execute_batch("ROLLBACK;").unwrap();
            timings.sort();eprintln!("active history reader, 100 collection commits: p50={:?}, p95={:?}, p99={:?}",timings[49],timings[94],timings[98]);
            timings[99]
        };
        {let reopened=Store::open(&path).unwrap();assert_eq!(reopened.conn.query_row("SELECT count(*) FROM snapshots",[],|row|row.get::<_,i64>(0)).unwrap(),101);}
        std::fs::remove_file(&path).unwrap();std::fs::remove_dir(&directory).unwrap();
        assert!(elapsed<std::time::Duration::from_secs(1),"collection waited {elapsed:?} for an unrelated history reader");
    }
    #[test]
    fn incremental_vacuum_finishes_a_page_batch_before_checkpointing() {
        let store=Store::open(":memory:").unwrap();
        store.conn.execute_batch("PRAGMA auto_vacuum=INCREMENTAL; VACUUM;
            CREATE TABLE vacuum_fixture(bytes BLOB);
            WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<1200)
            INSERT INTO vacuum_fixture SELECT zeroblob(4096) FROM n;
            DELETE FROM vacuum_fixture;").unwrap();
        let free=||store.conn.query_row("PRAGMA freelist_count",[],|row|row.get::<_,u64>(0)).unwrap();
        let before=free();assert!(before>512);
        store.reclaim_page_batch().unwrap();
        let after=free();assert!(before-after>=256,"only {} pages reclaimed",before-after);
        assert_eq!(store.conn.query_row("SELECT count(*) FROM vacuum_fixture",[],|row|row.get::<_,i64>(0)).unwrap(),0);
    }
    #[cfg(windows)]
    #[test]
    fn sensitive_values_do_not_create_false_changes_and_display_defaults_to_masked() {
        let mut store=Store::open(":memory:").unwrap();
        assert_eq!(store.retention().unwrap()["mode"],"unlimited");
        assert!(!store.show_sensitive().unwrap());
        store.set_show_sensitive(true).unwrap();assert!(store.show_sensitive().unwrap());
        store.set_show_sensitive(false).unwrap();assert!(!store.show_sensitive().unwrap());
        let a=pcscope_contracts::sensitive::protect(&json!("fixture-password")).unwrap();
        let b=pcscope_contracts::sensitive::protect(&json!("fixture-password")).unwrap();
        assert_eq!(store.ingest(&snapshot("encrypted-a",true,json!([{"id":"a","data":{"password":a}}]))).unwrap(),1);
        assert_eq!(store.ingest(&snapshot("encrypted-b",true,json!([{"id":"a","data":{"password":b}}]))).unwrap(),0);
        assert!(store.compare("test","machine","encrypted-a","encrypted-b").unwrap().is_empty());
        let c=pcscope_contracts::sensitive::protect(&json!("changed-fixture")).unwrap();
        assert_eq!(store.ingest(&snapshot("encrypted-c",true,json!([{"id":"a","data":{"password":c}}]))).unwrap(),1);
    }
    use super::*;
    #[test]
    fn event_items_classify_provider_id_status_and_filter_before_paging() {
        let s=Store::open(":memory:").unwrap();
        let security="pcscope.eventlog.security";
        let sysmon="pcscope.eventlog.microsoft-windows-sysmon/operational";
        for (source,kind,provider,id,suffix,action) in [
            (security,"execution.process_started","Microsoft-Windows-Security-Auditing",4688,"processes","execution.process_started"),
            (security,"identity.logon","Microsoft-Windows-Security-Auditing",4624,"sessions","identity.logon"),
            (security,"windows.event","wrong-provider",4688,"other","windows.event"),
            (security,"windows.event","Microsoft-Windows-Security-Auditing",99999,"other","windows.event"),
            (security,"coverage.security_log_cleared","Microsoft-Windows-Eventlog",1102,"log-clear","coverage.security_log_cleared"),
            (security,"coverage.gap","",0,"status","coverage.gap"),
            (sysmon,"coverage.recovered","",0,"status","coverage.recovered"),
            (sysmon,"execution.sysmon_event","Microsoft-Windows-Sysmon",1,"processes","execution.process_started"),
            (sysmon,"execution.sysmon_event","Microsoft-Windows-Sysmon",22,"dns","network.dns_query"),
            (sysmon,"execution.sysmon_event","Microsoft-Windows-Sysmon",255,"monitor","sysmon.error"),
        ] {
            s.conn.execute("INSERT INTO records(source,kind,object_id,occurred_at,data) VALUES(?,?,'','2026-09-11T00:00:00Z',?)",params![source,kind,json!({"provider":provider,"event_id":id}).to_string()]).unwrap();
            let item=format!("{source}.{suffix}");
            let row=s.search_timeline(&json!({}),None,1).unwrap();
            assert_eq!(row["items"][0]["item"],item);
            assert_eq!(row["items"][0]["action"],action);
            assert_eq!(row["items"][0]["kind"],kind,"original event type must remain intact");
        }
        let result=s.search_timeline(&json!({"source":sysmon,"action":"execution.process_started"}),None,1).unwrap();
        assert_eq!(result["items"].as_array().unwrap().len(),1);
        assert_eq!(result["items"][0]["source"],sysmon);
        assert_eq!(result["more"],false);
        s.conn.execute("UPDATE records SET data=? WHERE source=? AND json_extract(data,'$.event_id')=22",params![json!({"provider":"Microsoft-Windows-Sysmon","event_id":3}).to_string(),sysmon]).unwrap();
        assert_eq!(s.search_timeline(&json!({"item":format!("{sysmon}.dns")}),None,100).unwrap()["items"],json!([]));
        assert_eq!(s.search_timeline(&json!({"action":"network.connection_observed"}),None,100).unwrap()["items"].as_array().unwrap().len(),1);
    }
    #[test]
    fn gap_transitions_repeat_recover_and_recur_without_clearing_partial_sources() {
        let mut store=Store::open(":memory:").unwrap();
        let gap=|id:&str,reason:&str|message(json!({"protocol_version":1,"request_id":id,"type":"gap","source":"test","observed_at":id,"reason":reason}));
        let a=gap("2026-09-11T00:00:00Z","channel missing");
        assert_eq!(store.ingest(&a).unwrap(),1);
        assert_eq!(store.ingest(&a).unwrap(),0);
        assert_eq!(store.ingest(&gap("2026-09-11T00:01:00Z","channel missing")).unwrap(),0);
        assert_eq!(store.ingest(&gap("2026-09-11T00:02:00Z","access denied")).unwrap(),1);
        assert_eq!(store.ingest(&gap("2026-09-11T00:03:00Z","channel missing")).unwrap(),0);
        let success=snapshot("recovery",true,json!([]));
        store.ingest(&success).unwrap();
        assert_eq!(store.observe_collection_success(&[success.clone(),gap("partial","channel missing")]).unwrap(),0);
        assert_eq!(store.observe_collection_success(&[success.clone()]).unwrap(),1);
        assert_eq!(store.observe_collection_success(&[success]).unwrap(),0);
        let recovery=store.timeline(None,1).unwrap();
        assert_eq!(recovery[0]["kind"],"coverage.recovered");
        assert_eq!(recovery[0]["data"]["previous_gaps"].as_array().unwrap().len(),2);
        assert_eq!(store.ingest(&gap("2026-09-11T00:04:00Z","channel missing")).unwrap(),1);
    }

    #[test]
    fn gap_state_survives_restart_and_history_removal() {
        let dir=std::env::temp_dir().join(format!("pcscope-gap-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&dir).unwrap();let path=dir.join("history.db");
        let gap=|id:&str|message(json!({"protocol_version":1,"request_id":id,"type":"gap","source":"sysmon","observed_at":id,"reason":"channel missing"}));
        {
            let mut s=Store::open(&path).unwrap();assert_eq!(s.ingest(&gap("first")).unwrap(),1);
            s.conn.execute("DELETE FROM records",[]).unwrap();
        }
        {
            let mut s=Store::open(&path).unwrap();assert_eq!(s.ingest(&gap("second")).unwrap(),0);
            let status:(String,i64)=s.conn.query_row("SELECT last_seen,observations FROM coverage_state WHERE source='sysmon'",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
            assert_eq!(status,("second".to_owned(),2));
            let batch=message(json!({"protocol_version":1,"request_id":"ok","type":"event_batch","source":"sysmon","epoch":"e","batch_seq":1,"checkpoint":"0","events":[]}));
            s.ingest(&batch).unwrap();assert_eq!(s.observe_collection_success(&[batch]).unwrap(),1);
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
    fn message(v: Value) -> Envelope {
        pcscope_contracts::parse(&v.to_string()).unwrap()
    }
    fn snapshot(run: &str, complete: bool, items: Value) -> Envelope {
        message(
            json!({"protocol_version":1,"request_id":run,"type":"snapshot","source":"test","scope":"machine","run_id":run,"observed_at":"2026-09-10T00:00:00Z","complete":complete,"items":items}),
        )
    }
    fn batch(cp: &str, data: i32) -> Envelope {
        message(
            json!({"protocol_version":1,"request_id":cp,"type":"event_batch","source":"log","epoch":"1","batch_seq":cp.parse::<u64>().unwrap(),"checkpoint":cp,"events":[{"id":"1","occurred_at":"2026-09-10T00:00:00Z","kind":"test.event","object_id":"x","data":data}]}),
        )
    }
    #[test]
    fn monitoring_and_task_evidence_preserve_uncertainty() {
        let mut store=Store::open(":memory:").unwrap();
        assert_eq!(store.monitoring_settings().unwrap()["configured"],false);
        store.set_monitoring_settings(&json!({"mode":"low","repair":false,"configured":true}),"2026-09-10T00:00:00Z").unwrap();
        assert_eq!(store.monitoring_settings().unwrap()["configured"],true);
        let snapshot=json!({"scope":"machine","before":{},"after":{},"before_observed_at":"2026-09-10T00:00:00Z"});
        store.conn.execute("INSERT INTO records(source,kind,object_id,occurred_at,data) VALUES('pcscope.startup.task-details','object.changed','\\MapsUpdateTask','2026-09-10T00:05:00Z',?)",[snapshot.to_string()]).unwrap();
        let seq=store.conn.last_insert_rowid();
        for (task,time) in [(r"\MapsUpdateTask","2026-09-10T00:02:00Z"),(r"\Other","2026-09-10T00:02:00Z"),(r"\MapsUpdateTask","2026-09-10T00:06:00Z")] {
            store.conn.execute("INSERT INTO records(source,kind,object_id,occurred_at,data) VALUES('pcscope.eventlog.security','startup.task_updated','Security/4702',?,?)",params![time,json!({"event_id":4702,"fields":{"TaskName":task,"SubjectUserName":"fixture"}}).to_string()]).unwrap();
        }
        let evidence=store.task_evidence(seq).unwrap();
        assert_eq!(evidence["status"],"candidates");
        assert_eq!(evidence["events"].as_array().unwrap().len(),1);
        assert_eq!(evidence["events"][0]["data"]["fields"]["SubjectUserName"],"fixture");
        store.conn.execute("UPDATE records SET data='{}' WHERE seq=?",[seq]).unwrap();
        assert_eq!(store.task_evidence(seq).unwrap()["status"],"unknown_interval");
    }
    #[test]
    fn file_watch_defaults_empty_validates_and_versions() {
        let mut store=Store::open(":memory:").unwrap();let time="2026-09-11T00:00:00Z";
        assert_eq!(store.file_watch_settings().unwrap(),json!({"files":[],"revision":0}));
        for path in ["relative.txt",r"C:\",r"C:\..\file.txt",r"\\server\file.txt",r"C:\*.txt",r"C:\file.txt:stream",r"C:\bad.\file"] {
            assert!(store.set_file_watch_settings(vec![path.into()],time).is_err(),"accepted {path}");
        }
        let saved=store.set_file_watch_settings(vec![r"C:\missing\file.txt".into(),r"c:\missing\FILE.txt".into()],time).unwrap();
        assert_eq!(saved["files"].as_array().unwrap().len(),1);assert_eq!(saved["revision"],1);
        assert_eq!(store.set_file_watch_settings(vec![r"C:\missing\file.txt".into()],time).unwrap()["revision"],1);
        assert_eq!(store.set_file_watch_settings(vec![],time).unwrap()["revision"],2);
    }
    #[test]
    fn new_program_rejects_unmarked_database_before_writing_and_reopens_its_own_data() {
        let root=std::env::temp_dir().join(format!("pcscope-new-program-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));std::fs::create_dir(&root).unwrap();
        let legacy=root.join("legacy.db");drop(Store::open(&legacy).unwrap());let before=std::fs::read(&legacy).unwrap();
        assert!(Store::open_program(&legacy).is_err());assert_eq!(std::fs::read(&legacy).unwrap(),before);
        let new=root.join("native.db");{let mut store=Store::open_program(&new).unwrap();let at="2026-10-12T00:00:00Z";
            let saved=store.set_runtime_watch_settings(false,vec!["SIHOST.EXE".into(),"sihost".into()],at).unwrap();assert_eq!(saved["module_process_names"],json!(["sihost"]));
            assert!(store.set_runtime_watch_settings(true,vec!["sihost*".into()],at).is_err());assert!(store.set_runtime_watch_settings(true,vec![r"C:\program.exe".into()],at).is_err());
            store.save_runtime_watch_state(&json!({"files":{},"revision":4,"format_version":1})).unwrap();}
        {let store=Store::open_program(&new).unwrap();assert_eq!(store.runtime_watch_settings().unwrap()["enabled"],false);assert_eq!(store.runtime_watch_state().unwrap()["revision"],4);assert_eq!(store.file_watch_settings().unwrap()["files"],json!([]));}
        #[cfg(windows)] {let alias=root.join("linked.db");std::fs::hard_link(&new,&alias).unwrap();assert!(Store::open_program(&alias).is_err());std::fs::remove_file(alias).unwrap();}
        for file in std::fs::read_dir(&root).unwrap(){std::fs::remove_file(file.unwrap().path()).unwrap();}std::fs::remove_dir(root).unwrap();
    }
    #[test]
    fn search_index_backfills_updates_prunes_and_uses_item_index() {
        let mut store=Store::open(":memory:").unwrap();
        store.conn.execute_batch("DROP TRIGGER records_search_insert;DROP TRIGGER records_search_update;DROP TABLE record_search;DROP TABLE collection_definitions;DELETE FROM settings WHERE key='search_catalog_v1';PRAGMA user_version=4;
            INSERT INTO records(source,kind,object_id,occurred_at,data) VALUES('pcscope.software','object.changed','fixture','2026-09-11T00:00:00Z','{}');").unwrap();
        search_index::ensure(&mut store.conn).unwrap();
        let result=store.search_timeline(&json!({"item":"pcscope.software"}),None,100).unwrap();
        assert_eq!(result["items"].as_array().unwrap().len(),1);
        let plan:String=store.conn.query_row("EXPLAIN QUERY PLAN SELECT seq FROM record_search WHERE item='pcscope.software' AND seq<999 ORDER BY seq DESC LIMIT 101",[],|r|r.get(3)).unwrap();
        assert!(plan.contains("record_search_item"),"{plan}");
        store.conn.execute("UPDATE records SET source='custom.source',kind='custom.event'",[]).unwrap();
        let result=store.search_timeline(&json!({"kind":"events"}),None,100).unwrap();
        assert_eq!(result["items"][0]["item"],"custom.source");
        assert_eq!(result["sources"].as_array().unwrap().len(),1);
        store.conn.execute("DELETE FROM records",[]).unwrap();
        assert_eq!(store.search_timeline(&json!({}),None,100).unwrap()["sources"],json!([]));
    }
    #[test]
    fn wal_reader_does_not_block_writer_and_queries_are_interruptible() {
        let dir=std::env::temp_dir().join(format!("pcscope-query-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&dir).unwrap();let path=dir.join("history.db");
        {
            let mut writer=Store::open(&path).unwrap();
            let reader=Store::open_reader(&path,std::time::Duration::from_secs(30)).unwrap();
            let tx=reader.conn.unchecked_transaction().unwrap();
            let before:i64=tx.query_row("SELECT count(*) FROM records",[],|r|r.get(0)).unwrap();
            writer.internal_event("collector.test","2026-09-11T00:00:00Z",json!({})).unwrap();
            assert_eq!(tx.query_row("SELECT count(*) FROM records",[],|r|r.get::<_,i64>(0)).unwrap(),before);
            tx.commit().unwrap();
            assert_eq!(reader.conn.query_row("SELECT count(*) FROM records",[],|r|r.get::<_,i64>(0)).unwrap(),before+1);
            assert!(reader.conn.execute("DELETE FROM records",[]).is_err());
            let limited=Store::open_reader(&path,std::time::Duration::ZERO).unwrap();
            let result=limited.conn.query_row("WITH RECURSIVE n(x) AS (VALUES(0) UNION ALL SELECT x+1 FROM n WHERE x<1000000) SELECT sum(x) FROM n",[],|r|r.get::<_,i64>(0));
            assert!(result.unwrap_err().to_string().contains("interrupted"));
            assert!(writer.internal_event("collector.test","2026-09-11T00:00:01Z",json!({})).is_ok());
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
    #[test]
    fn collection_filters_match_subitems_and_history_before_paging() {
        let s = Store::open(":memory:").unwrap();
        let fixtures = [
            (
                "pcscope.shell.registrations",
                r"software\microsoft\windows\currentversion\previewhandlers",
                "第三方预览处理程序",
                "pcscope.shell.registrations.preview",
            ),
            (
                "pcscope.shell.registrations",
                r"software\classes\clsid\{abc}\inprocserver32",
                "COM 实现与扩展文件元数据",
                "pcscope.shell.registrations.com",
            ),
            (
                "pcscope.shell.registrations",
                r"software\classes\*\shell\open",
                "右键菜单、文件关联与默认图标",
                "pcscope.shell.registrations.file-association",
            ),
            (
                "pcscope.startup.logon",
                r"c:\users\test\startup\demo.lnk",
                "用户与公共启动文件夹",
                "pcscope.startup.logon.folders",
            ),
            (
                "pcscope.startup.logon",
                "LocalMachine/Registry64/Run/demo",
                "登录启动登记",
                "pcscope.startup.logon.registry",
            ),
            (
                "third-party.unknown",
                "unknown",
                "third-party.unknown",
                "third-party.unknown",
            ),
        ];
        for (source, id, name, item) in fixtures {
            s.conn
                .execute(
                    "INSERT INTO objects VALUES(?,'scope',?,'{}')",
                    params![source, id],
                )
                .unwrap();
            s.conn.execute("INSERT INTO records(source,kind,object_id,occurred_at,data) VALUES(?,'object.changed',?,'2026-09-10T00:00:00Z','{}')",params![source,id]).unwrap();
            let object = s.search_objects(&json!({"item":item}), 0, 1).unwrap();
            assert_eq!(object["total"], 1);
            assert_eq!(object["items"][0]["item_name"], name);
            let event = s.search_timeline(&json!({"item":item}), None, 1).unwrap();
            assert_eq!(event["items"][0]["item_name"], name);
        }
        for _ in 0..105 {
            s.conn.execute("INSERT INTO records(source,kind,object_id,occurred_at,data) VALUES('pcscope.software','object.changed','new','2026-09-10T00:00:00Z','{}')",[]).unwrap();
        }
        let old = s
            .search_timeline(&json!({"search":"第三方预览处理程序"}), None, 1)
            .unwrap();
        assert_eq!(old["items"].as_array().unwrap().len(), 1);
        assert_eq!(old["more"], false);
        let first = s
            .search_timeline(&json!({"item":"pcscope.software"}), None, 100)
            .unwrap();
        assert_eq!(first["more"], true);
        let second = s
            .search_timeline(
                &json!({"item":"pcscope.software"}),
                first["items"][99]["seq"].as_i64(),
                100,
            )
            .unwrap();
        assert_eq!(second["items"].as_array().unwrap().len(), 5);
        assert_eq!(second["more"], false);
        assert_eq!(
            s.search_timeline(&json!({"search":"' OR 1=1 --"}), None, 100)
                .unwrap()["items"],
            json!([])
        );
        s.conn.execute("INSERT INTO records(source,kind,object_id,occurred_at,data) VALUES('pcscope.shell.registrations','coverage.gap','','2026-09-10T00:00:00Z','{}')",[]).unwrap();
        let gaps = s
            .search_timeline(&json!({"kind":"coverage"}), None, 100)
            .unwrap();
        assert_eq!(gaps["items"][0]["item"], "pcscope.shell.registrations");
        assert_eq!(
            s.search_timeline(&json!({"kind":"events"}), None, 100)
                .unwrap()["items"],
            json!([])
        );
    }

    #[test]
    fn object_search_filters_before_paging_and_binds_literals() {
        let mut s = Store::open(":memory:").unwrap();
        let items:Vec<Value>=(0..205).map(|i|json!({"id":format!("item-{i:03}"),"data":{"Name":format!("Object {i:03}"),"version":if i==204{"needle 中文 100%_"}else{"normal"}}})).collect();
        let mut snapshot = snapshot("search", true, json!(items));
        if let Message::Snapshot { source, .. } = &mut snapshot.message {
            *source = "pcscope.software".into();
        }
        s.ingest(&snapshot).unwrap();
        let all = s.search_objects(&json!({}), 100, 100).unwrap();
        assert_eq!(all["total"], 205);
        assert_eq!(all["items"].as_array().unwrap().len(), 100);
        let found=s.search_objects(&json!({"search":"NEEDLE 中文 100%_","category":"software","source":"pcscope.software"}),0,100).unwrap();
        assert_eq!(found["total"], 1);
        assert_eq!(found["items"][0]["id"], "item-204");
        assert_eq!(
            s.search_objects(&json!({"search":"' OR 1=1 --"}), 0, 100)
                .unwrap()["total"],
            0
        );
        assert_eq!(
            s.search_objects(&json!({"category":"network"}), 0, 100)
                .unwrap()["total"],
            0
        );
        assert_eq!(
            s.search_objects(&json!({"sort":"name_desc"}), 0, 1)
                .unwrap()["items"][0]["id"],
            "item-204"
        );
        assert!(s
            .search_objects(&json!({"sort":"DROP TABLE objects"}), 0, 100)
            .is_err());
        assert!(s
            .search_objects(&json!({"since":"bad-date"}), 0, 100)
            .is_err());
        assert_eq!(
            s.search_objects(&json!({}), 9999, 100).unwrap()["offset"],
            200
        );
    }
    #[test]
    fn object_times_track_changes_separately_and_survive_history_pruning() {
        let mut s = Store::open(":memory:").unwrap();
        for (run, date, value, complete) in [
            ("1", "2026-09-08T00:00:00Z", 1, true),
            ("2", "2026-09-09T00:00:00Z", 1, true),
            ("3", "2026-09-10T00:00:00Z", 2, false),
        ] {
            let mut envelope = snapshot(run, complete, json!([{"id":"a","data":value}]));
            if let Message::Snapshot { observed_at, .. } = &mut envelope.message {
                *observed_at = date.into();
            }
            assert_eq!(s.ingest(&envelope).is_ok(), complete);
        }
        let result = s.search_objects(&json!({}), 0, 100).unwrap();
        assert_eq!(result["items"][0]["changed_at"], "2026-09-08T00:00:00Z");
        assert_eq!(result["items"][0]["seen_at"], "2026-09-09T00:00:00Z");
        assert_eq!(
            s.search_objects(&json!({"since":"2026-09-09T00:00:00Z"}), 0, 100)
                .unwrap()["total"],
            0
        );
        s.conn.execute("DELETE FROM records", []).unwrap();
        s.conn.execute("DELETE FROM snapshots", []).unwrap();
        assert_eq!(
            s.search_objects(&json!({}), 0, 100).unwrap()["items"][0]["changed_at"],
            "2026-09-08T00:00:00Z"
        );
        s.ingest(&snapshot("4", true, json!([{"id":"a","data":2}])))
            .unwrap();
        assert_eq!(
            s.search_objects(&json!({"since":"2026-09-09T00:00:00Z"}), 0, 100)
                .unwrap()["total"],
            1
        );
        s.ingest(&snapshot("5", true, json!([]))).unwrap();
        assert_eq!(s.search_objects(&json!({}), 0, 100).unwrap()["total"], 0);
        assert_eq!(
            s.conn
                .query_row("SELECT count(*) FROM object_times", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
    #[test]
    fn replay_is_idempotent_and_conflicts_rollback() {
        let mut s = Store::open(":memory:").unwrap();
        assert_eq!(s.ingest(&batch("1", 1)).unwrap(), 1);
        assert_eq!(s.ingest(&batch("1", 1)).unwrap(), 0);
        assert!(s.ingest(&batch("2", 2)).is_err());
        assert_eq!(s.checkpoint("log", "1").unwrap().as_deref(), Some("1"));
        assert_eq!(s.timeline(None, 100).unwrap().len(), 1);
    }
    #[test]
    fn incomplete_snapshot_does_not_delete_and_replay_does_not_duplicate() {
        let mut s = Store::open(":memory:").unwrap();
        let a = snapshot("1", true, json!([{"id":"a","data":{"v":1}}]));
        assert_eq!(s.ingest(&a).unwrap(), 1);
        assert_eq!(s.ingest(&a).unwrap(), 0);
        assert!(s.ingest(&snapshot("2", false, json!([]))).is_err());
        assert_eq!(
            s.ingest(&snapshot("3", true, json!([{"id":"a","data":{"v":2}}])))
                .unwrap(),
            1
        );
        assert_eq!(s.ingest(&snapshot("4", true, json!([]))).unwrap(), 1);
        let rows = s.timeline(None, 100).unwrap();
        assert_eq!(rows[0]["kind"], "object.no_longer_observed");
        assert_eq!(rows[2]["kind"], "baseline.observed");
    }
    #[test]
    fn scope_change_does_not_delete_other_scope() {
        let mut s = Store::open(":memory:").unwrap();
        s.ingest(&snapshot("1", true, json!([{"id":"a","data":1}])))
            .unwrap();
        let mut other = snapshot("2", true, json!([]));
        if let Message::Snapshot { scope, .. } = &mut other.message {
            *scope = "user".into();
        }
        assert_eq!(s.ingest(&other).unwrap(), 0);
    }
    #[test]
    fn durable_reopen_preserves_checkpoint_and_history() {
        let directory = std::env::temp_dir().join(format!(
            "pcscope-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("history.db");
        {
            let mut s = Store::open(&path).unwrap();
            s.save_plugin_settings(&json!({"scoped":{"enabled":false,"seconds":777}}))
                .unwrap();
            s.ingest(&batch("1", 7)).unwrap();
        }
        {
            let mut s = Store::open(&path).unwrap();
            assert_eq!(s.plugin_settings().unwrap()["scoped"]["seconds"], 777);
            assert_eq!(s.plugin_settings().unwrap()["scoped"]["enabled"], false);
            assert_eq!(s.checkpoint("log", "1").unwrap().as_deref(), Some("1"));
            assert_eq!(s.ingest(&batch("1", 7)).unwrap(), 0);
            assert_eq!(s.timeline(None, 10).unwrap().len(), 1);
        }
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(&directory).unwrap();
    }
    #[test]
    fn collision_rolls_back_earlier_events_in_same_batch() {
        let mut s = Store::open(":memory:").unwrap();
        s.ingest(&batch("1", 1)).unwrap();
        let mut conflicting = batch("2", 2);
        if let Message::EventBatch { events, .. } = &mut conflicting.message {
            let mut first = events[0].clone();
            first.id = "new".into();
            events.insert(0, first);
        }
        assert!(s.ingest(&conflicting).is_err());
        assert_eq!(s.timeline(None, 100).unwrap().len(), 1);
        assert_eq!(s.checkpoint("log", "1").unwrap().as_deref(), Some("1"));
    }
    #[test]
    fn stale_batch_cannot_regress_checkpoint() {
        let mut s = Store::open(":memory:").unwrap();
        s.ingest(&batch("1", 1)).unwrap();
        assert!(s.ingest(&batch("3", 1)).is_err());
        s.ingest(&batch("2", 1)).unwrap();
        assert!(s.ingest(&batch("1", 1)).is_err());
        assert_eq!(s.checkpoint("log", "1").unwrap().as_deref(), Some("2"));
    }
    #[test]
    fn rolling_reclaims_disk_preserves_state_and_dedup_receipts() {
        let directory = std::env::temp_dir().join(format!(
            "pcscope-capacity-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("history.db");
        {
            let mut s = Store::open(&path).unwrap();
            s.set_retention("rolling", MIN_QUOTA).unwrap();
            let first = snapshot(
                "0",
                true,
                json!([{"id":"a","data":{"version":0,"padding":"x".repeat(250000)}}]),
            );
            s.ingest(&first).unwrap();
            for n in 1..48 {
                s.ingest(&snapshot(
                    &n.to_string(),
                    true,
                    json!([{"id":"a","data":{"version":n,"padding":"x".repeat(250000)}}]),
                ))
                .unwrap();
                assert!(s.physical_bytes().unwrap() <= MIN_QUOTA);
            }
            assert!(s.retention().unwrap()["pruned_units"].as_u64().unwrap() > 0);
            assert_eq!(s.objects(0, 10).unwrap()[0]["data"]["version"], 47);
            assert_eq!(s.ingest(&first).unwrap(), 0);
            assert_eq!(s.objects(0, 10).unwrap()[0]["data"]["version"], 47);
            assert!(s.compare("test", "machine", "0", "47").is_err());
            assert_eq!(s.timeline(None, 1).unwrap()[0]["kind"], "object.changed");
        }
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
    #[test]
    fn unlimited_keeps_history_beyond_configured_quota() {
        let mut s = Store::open(":memory:").unwrap();
        s.set_retention("unlimited", MIN_QUOTA).unwrap();
        for n in 0..48 {
            s.ingest(&snapshot(
                &n.to_string(),
                true,
                json!([{"id":"a","data":{"version":n,"padding":"x".repeat(250000)}}]),
            ))
            .unwrap();
        }
        assert!(s.live_bytes().unwrap() > MIN_QUOTA);
        assert_eq!(s.counts().unwrap()["snapshots"], 48);
        assert_eq!(s.counts().unwrap()["records"], 48);
        assert_eq!(s.retention().unwrap()["pruned_units"], 0);
        assert_eq!(s.compare("test", "machine", "0", "47").unwrap().len(), 1);
    }
    #[test]
    fn lifecycle_distinguishes_clean_exit_from_interruption() {
        let mut s = Store::open(":memory:").unwrap();
        s.begin_session("2026-09-10T00:00:00Z").unwrap();
        s.heartbeat("2026-09-10T00:01:00Z").unwrap();
        s.begin_session("2026-09-10T00:02:00Z").unwrap();
        assert_eq!(
            s.timeline(None, 100)
                .unwrap()
                .iter()
                .filter(|r| r["kind"] == "coverage.unclean_shutdown")
                .count(),
            1
        );
        s.end_session("2026-09-10T00:03:00Z").unwrap();
        s.begin_session("2026-09-10T00:04:00Z").unwrap();
        assert_eq!(
            s.timeline(None, 100)
                .unwrap()
                .iter()
                .filter(|r| r["kind"] == "coverage.unclean_shutdown")
                .count(),
            1
        );
        assert_eq!(
            s.source_resume("pcscope.core").unwrap().unwrap()["batch_seq"],
            5
        );
    }

    // ---------- S05 evidence ingress ----------

    fn count(s: &Store, sql: &str) -> i64 {
        s.conn.query_row(sql, [], |r| r.get(0)).unwrap()
    }
    fn evidence_count(s: &Store) -> i64 {
        count(s, "SELECT count(*) FROM evidence")
    }
    fn pending_count(s: &Store) -> i64 {
        count(s, "SELECT count(*) FROM projection_queue WHERE state='pending'")
    }
    fn event(id: &str, epoch: &str, data: Value) -> Envelope {
        message(
            json!({"protocol_version":1,"request_id":format!("{epoch}-{id}"),"type":"event_batch","source":"log","epoch":epoch,"batch_seq":1,"checkpoint":format!("cp-{epoch}"),"events":[{"id":id,"occurred_at":"2026-09-10T00:00:00Z","kind":"test.event","object_id":"x","data":data}]}),
        )
    }

    #[test]
    fn replaying_a_batch_registers_neither_evidence_nor_work_twice() {
        let mut store = Store::open(":memory:").unwrap();
        let envelope = snapshot("run-1", true, json!([{"id":"a","data":{"name":"A"}},{"id":"b","data":{"name":"B"}}]));
        // Two items on a first baseline: one derived record each.
        assert_eq!(store.ingest(&envelope).unwrap(), 2);
        assert_eq!(evidence_count(&store), 2);
        assert_eq!(pending_count(&store), 2);
        // The same run again changes nothing anywhere, not just in the raw table.
        assert_eq!(store.ingest(&envelope).unwrap(), 0);
        assert_eq!(evidence_count(&store), 2);
        assert_eq!(pending_count(&store), 2);

        let first = event("1", "1", json!({"value":1}));
        assert_eq!(store.ingest(&first).unwrap(), 1);
        assert_eq!(evidence_count(&store), 3);
        assert_eq!(store.ingest(&first).unwrap(), 0);
        assert_eq!(evidence_count(&store), 3);
        assert_eq!(pending_count(&store), 3);
    }

    #[test]
    fn a_failed_transaction_advances_nothing() {
        let mut store = Store::open(":memory:").unwrap();
        assert_eq!(store.ingest(&event("1", "1", json!({"value":1}))).unwrap(), 1);
        assert_eq!(store.checkpoint("log", "1").unwrap().as_deref(), Some("cp-1"));
        let before = evidence_count(&store);
        // The same event identity with different content is refused, and the batch carries a
        // checkpoint that must not be reached.
        let mut conflicting = event("1", "1", json!({"value":2}));
        if let Message::EventBatch { checkpoint, .. } = &mut conflicting.message {
            *checkpoint = "cp-must-not-land".into();
        }
        assert!(store.ingest(&conflicting).is_err());
        assert_eq!(evidence_count(&store), before);
        assert_eq!(store.checkpoint("log", "1").unwrap().as_deref(), Some("cp-1"));
    }

    #[test]
    fn a_reset_log_reuses_an_event_id_under_a_new_epoch() {
        let mut store = Store::open(":memory:").unwrap();
        assert_eq!(store.ingest(&event("1", "1", json!({"value":"before-reset"}))).unwrap(), 1);
        // The log is cleared and starts again at id 1. The evidence address carries the epoch, so
        // the new event is a different address rather than a collision with the old one.
        assert_eq!(store.ingest(&event("1", "2", json!({"value":"after-reset"}))).unwrap(), 1);
        let addresses: Vec<String> = {
            let mut q = store.conn.prepare("SELECT evidence_id FROM evidence ORDER BY seq").unwrap();
            let rows = q.query_map([], |r| r.get(0)).unwrap();
            rows.collect::<rusqlite::Result<_>>().unwrap()
        };
        assert_eq!(addresses, vec!["log|1|1".to_owned(), "log|2|1".to_owned()]);
    }

    #[test]
    fn evidence_points_at_the_material_instead_of_copying_it() {
        let mut store = Store::open(":memory:").unwrap();
        let marker = "fixture-secret-that-must-not-be-copied";
        assert_eq!(store.ingest(&snapshot("run-1", true, json!([{"id":"a","data":{"blob":marker}}]))).unwrap(), 1);  // one item, one record
        assert_eq!(store.ingest(&event("1", "1", json!({"blob":marker}))).unwrap(), 1);
        // One row per item, and none of them carries the payload: it lives in `records`/`snapshots`.
        let rows: Vec<(String, String, String)> = {
            let mut q = store.conn.prepare("SELECT evidence_id,item_id,payload_ref FROM evidence").unwrap();
            let rows = q.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
            rows.collect::<rusqlite::Result<_>>().unwrap()
        };
        assert_eq!(rows.len(), 2);
        for row in &rows {
            let joined = format!("{}{}{}", row.0, row.1, row.2);
            assert!(!joined.contains(marker), "evidence row copies the payload: {joined}");
        }
        // The material is still handed back, read out of where it was stored once.
        let inputs = store.evidence_inputs(0, 10).unwrap();
        assert_eq!(inputs.len(), 2);
        assert_eq!(inputs[0].payload["blob"], json!(marker));
        assert_eq!(inputs[1].payload["blob"], json!(marker));
    }

    #[test]
    fn the_three_times_stay_apart() {
        let mut store = Store::open(":memory:").unwrap();
        let mut envelope = snapshot("run-1", true, json!([{"id":"a","data":{"name":"A"}}]));
        if let Message::Snapshot { observed_at, .. } = &mut envelope.message {
            *observed_at = "2026-09-01T00:00:00Z".into();
        }
        store.ingest(&envelope).unwrap();
        store.ingest(&event("1", "1", json!({"value":1}))).unwrap();
        let inputs = store.evidence_inputs(0, 10).unwrap();

        // A snapshot declares when the collector looked; it states no separate source time.
        assert_eq!(inputs[0].observed_at, "2026-09-01T00:00:00Z");
        assert_eq!(inputs[0].source_time, None);
        // An event states when it happened. The batch carries no observation time of its own, so
        // the receiving time stands in and is not dressed up as something the source said.
        assert_eq!(inputs[1].source_time.as_deref(), Some("2026-09-10T00:00:00Z"));
        assert_ne!(inputs[1].observed_at, "2026-09-10T00:00:00Z");
        // Everything records when it reached the database, which is neither of the above.
        assert!(inputs[0].ingested_at.ends_with('Z'));
        assert!(inputs[1].ingested_at.ends_with('Z'));
        assert_ne!(inputs[0].ingested_at, "2026-09-01T00:00:00Z");

        // And the shape is the one adapters were promised, not a near miss.
        assert_eq!(inputs[0].source, "test");
        assert_eq!(inputs[0].item_id, "a");
        assert_eq!(inputs[0].native_record_id, "a");
        assert_eq!(inputs[0].structure_version, crate::evidence::PROTOCOL_STRUCTURE_VERSION);
        assert_eq!(inputs[0].evidence, pcscope_contracts::adapter::EvidenceRef::snapshot("test", "machine", "run-1"));
        assert_eq!(inputs[1].evidence, pcscope_contracts::adapter::EvidenceRef::event("log", "1", "1"));
    }

    #[test]
    fn the_queue_reports_what_is_outstanding() {
        let mut store = Store::open(":memory:").unwrap();
        store.ingest(&snapshot("run-1", true, json!([{"id":"a","data":{}},{"id":"b","data":{}}]))).unwrap();
        let summary = store.evidence_queue(crate::evidence::OBJECT_PROJECTION).unwrap();
        assert_eq!(summary["total"], 2);
        assert_eq!(summary["by_state"]["pending"], 2);
    }
}

#[cfg(test)]
mod s08_tests {
    use super::*;

    const TASKS: &str = "pcscope.startup.tasks";
    const TASK_SCOPE: &str = "machine:m/install:i/schema:1/context:/shard:0";
    const TASK_PATH: &str = r"\例子文件夹\例子任务";
    const MACHINE: &str = "m";
    const INSTALL: &str = "i";

    fn envelope(value: Value) -> Envelope {
        pcscope_contracts::parse(&value.to_string()).unwrap()
    }

    fn tasks_snapshot(run: &str, observed_at: &str, name: &str) -> Envelope {
        envelope(json!({
            "protocol_version": 1, "request_id": run, "type": "snapshot",
            "source": TASKS, "scope": TASK_SCOPE, "run_id": run, "observed_at": observed_at, "complete": true,
            "items": [{"id": TASK_PATH, "data": {"path": TASK_PATH, "name": name, "enabled": true}}]
        }))
    }

    fn gap(source: &str, at: &str) -> Envelope {
        envelope(json!({
            "protocol_version": 1, "request_id": format!("gap-{at}"), "type": "gap",
            "source": source, "reason": "access denied", "observed_at": at
        }))
    }

    fn count(store: &Store, sql: &str) -> i64 {
        store.conn.query_row(sql, [], |r| r.get(0)).unwrap()
    }

    fn coverage(store: &Store, source: &str) -> Value {
        store
            .coverage_report()
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["source"] == source)
            .cloned()
            .unwrap_or(Value::Null)
    }

    /// The control that covers the scheduled-task source, as the controls page knows it.
    fn task_control() -> (&'static str, &'static str) {
        ("pcscope-powershell-adapter.exe", "pcscope.startup.tasks")
    }

    #[test]
    fn a_disabled_item_keeps_its_facts_and_the_time_they_were_observed() {
        let mut store = Store::open(":memory:").unwrap();
        store.ingest(&tasks_snapshot("run-1", "2026-09-12T00:00:00Z", "示例任务")).unwrap();
        store.project_pending(MACHINE, INSTALL, 10).unwrap();
        let before: (String, String) = store
            .conn
            .query_row("SELECT value,observed_at FROM object_facts WHERE field_id='name'", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();

        // The item is switched off. Nothing was collected for it afterwards, and that silence is
        // not evidence that the object went away.
        let (plugin, item) = task_control();
        store.set_item_enabled(plugin, item, false, "2026-09-12T01:00:00Z").unwrap();
        let hints = pcscope_contracts::identity::LifecycleHints {
            complete_observation: true,
            collected_scope: pcscope_contracts::semantics::business_scope(TASK_SCOPE),
            configured: false,
            history_retained_from: None,
            states_object_ended: false,
            boot_id: None,
        };
        let decided = store.conclude_absence(TASKS, &pcscope_contracts::semantics::business_scope(TASK_SCOPE), &hints, "2026-09-12T01:00:00Z").unwrap();
        assert!(decided.is_empty(), "a switched-off item's silence is not an absence");
        let after: (String, String) = store
            .conn
            .query_row("SELECT value,observed_at FROM object_facts WHERE field_id='name'", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert_eq!(before, after, "the retained value and its real observation time both survive");
    }

    #[test]
    fn re_enabling_does_not_claim_the_missing_interval_was_covered() {
        let mut store = Store::open(":memory:").unwrap();
        store.ingest(&tasks_snapshot("run-1", "2026-09-12T00:00:00Z", "示例任务")).unwrap();
        assert_eq!(coverage(&store, TASKS)["history_state"], "caught_up");
        assert!(coverage(&store, TASKS)["configured"].as_bool().unwrap());

        // While an item is off nothing is collected for it, so the switch itself is the only moment
        // at which the missing interval can be recorded.
        let (plugin, item) = task_control();
        store.set_item_enabled(plugin, item, false, "2026-09-12T01:00:00Z").unwrap();
        let off = coverage(&store, TASKS);
        assert!(!off["configured"].as_bool().unwrap(), "a switched-off source must not read as merely quiet");

        store.set_item_enabled(plugin, item, true, "2026-09-12T02:00:00Z").unwrap();
        // Collection works again, and the interval it was off stays missing: recovering is not the
        // same as having observed it.
        store.ingest(&tasks_snapshot("run-3", "2026-09-12T02:00:00Z", "示例任务")).unwrap();
        let back = coverage(&store, TASKS);
        assert!(back["configured"].as_bool().unwrap());
        assert_eq!(back["history_state"], "gap", "recovery does not fill in what was never observed");
        let reasons: Vec<String> = {
            let mut q = store.conn.prepare("SELECT reason FROM coverage_state WHERE source=? AND active=1").unwrap();
            let rows = q.query_map([TASKS], |r| r.get(0)).unwrap();
            rows.collect::<rusqlite::Result<_>>().unwrap()
        };
        assert!(reasons.iter().any(|reason| reason == coverage::DISABLED_INTERVAL), "{reasons:?}");
    }

    #[test]
    fn one_failing_source_writes_one_row_not_one_per_object() {
        let mut store = Store::open(":memory:").unwrap();
        store.ingest(&gap(TASKS, "2026-09-12T00:00:00Z")).unwrap();
        store.ingest(&gap(TASKS, "2026-09-12T00:01:00Z")).unwrap();
        // The same source failing again is the same failure being observed again, not a new one.
        assert_eq!(count(&store, "SELECT count(*) FROM coverage_state WHERE source='pcscope.startup.tasks'"), 1);
        assert_eq!(count(&store, "SELECT count(*) FROM coverage_scopes"), 1);
        let row = coverage(&store, TASKS);
        assert_eq!(row["last_result"], "collection_failed");
        assert!(row["last_success_at"].is_null(), "a source that never succeeded has no success time to report");
    }

    #[test]
    fn pruning_marks_the_evidence_and_leaves_no_unexplained_reference() {
        let mut store = Store::open(":memory:").unwrap();
        store
            .ingest(&envelope(json!({
                "protocol_version": 1, "request_id": "b-1", "type": "event_batch",
                "source": "pcscope.eventlog.microsoft-windows-taskscheduler/operational",
                "epoch": "1", "batch_seq": 1, "checkpoint": "cp-1",
                "events": [{"id": "100", "occurred_at": "2026-09-12T00:00:00Z", "kind": "windows.event", "object_id": "100",
                            "data": {"provider": "Microsoft-Windows-TaskScheduler", "event_id": 100, "task_name": TASK_PATH}}]
            })))
            .unwrap();
        store.project_pending(MACHINE, INSTALL, 10).unwrap();
        // The action row cites its evidence; a second row under the occurrence key is the record that
        // keeps a replay from writing the occurrence twice.
        assert_eq!(count(&store, "SELECT count(*) FROM evidence_refs WHERE subject_kind='action' AND subject_id=CAST((SELECT action_row FROM object_actions LIMIT 1) AS TEXT)"), 1);

        let unit: i64 = store.conn.query_row("SELECT unit_id FROM records ORDER BY seq LIMIT 1", [], |r| r.get(0)).unwrap();
        let marked = evidence::mark_pruned_for_unit(&store.conn, unit, "2026-09-12T02:00:00Z").unwrap();
        assert_eq!(marked, 1, "the evidence whose material this unit holds is marked");
        assert_eq!(count(&store, "SELECT count(*) FROM evidence_refs WHERE pruned_at IS NULL"), 0, "no reference still points at pruned evidence");
        // The action itself is untouched: it keeps its own values and observation time, and the loss
        // is stated rather than left as a pointer into a row that is gone.
        assert_eq!(count(&store, "SELECT count(*) FROM object_actions"), 1);
        assert_eq!(count(&store, "SELECT count(*) FROM evidence WHERE pruned_at IS NOT NULL"), 1);
    }

    #[test]
    fn a_deep_projection_backlog_stops_ingest_with_a_clear_error() {
        let mut store = Store::open(":memory:").unwrap();
        store.conn.execute("UPDATE settings SET value='1' WHERE key='max_pending_projections'", []).unwrap();
        store.ingest(&tasks_snapshot("run-1", "2026-09-12T00:00:00Z", "甲")).unwrap();
        store.ingest(&tasks_snapshot("run-2", "2026-09-12T00:01:00Z", "乙")).unwrap();
        store.begin_session("2026-09-12T00:01:10Z").unwrap();
        store.end_session("2026-09-12T00:01:20Z").unwrap();
        // Two snapshots plus lifecycle records are queued. Another snapshot is refused rather
        // than accepted and pruned: what would be lost is the raw evidence the model is built on.
        let error = store.ingest(&tasks_snapshot("run-3", "2026-09-12T00:02:00Z", "丙")).unwrap_err().to_string();
        assert!(error.contains("projection backlog"), "{error}");
        let spoof = envelope(json!({"protocol_version":1,"request_id":"external-core","type":"event_batch",
            "source":"pcscope.core","epoch":"core-v1","batch_seq":3,"checkpoint":"3",
            "events":[{"id":"3","occurred_at":"2026-09-12T00:01:30Z","kind":"collector.started",
                "object_id":"pcscope.service","data":{}}]}));
        assert!(store.ingest(&spoof).unwrap_err().to_string().contains("projection backlog"),
            "claiming the internal source must not bypass the external-ingest guard");
        // Refusing rolled the batch back entirely, so the source can simply send it again.
        assert_eq!(count(&store, "SELECT count(*) FROM snapshots"), 2);
        store.project_pending(MACHINE, INSTALL, 10).unwrap();
        store.ingest(&tasks_snapshot("run-3", "2026-09-12T00:02:00Z", "丙")).unwrap();
        assert_eq!(count(&store, "SELECT count(*) FROM snapshots"), 3);
    }
}
