//! S05 evidence ingress (database schema version 7).
//!
//! What this adds on top of the raw store: every piece of persisted material gets a **stable
//! evidence address** at the moment it is ingested, plus a projection work item, in the same
//! transaction as the material itself. A batch that fails to commit leaves no evidence, no queue
//! entry and no advanced checkpoint, so the source can simply be replayed.
//!
//! The payload is never copied here. `payload_ref` says where the material already lives — a row in
//! `records`, or a snapshot in `snapshots` — so a large inventory or a block of XML is stored once
//! and referenced by every field that cites it, which is what §7.3 of the refactor plan requires.
//!
//! `observed_at`, `source_time` and `ingested_at` are three different questions and are stored
//! separately, per the R04 semantic decisions:
//!   * `source_time` — the time the source attributes to the fact (an event's `occurred_at`);
//!   * `observed_at` — when PCScope saw it (a snapshot declares this; an event batch does not, so
//!     the receiving time is used and stands in, rather than being dressed up as a source time);
//!   * `ingested_at` — when it reached this database.
//!
//! `crates/contracts/src/reference.rs::evidence_from` builds the same shape from fixture lines and
//! says it is what S05 would produce from the store. This module is that: `evidence_inputs` reads
//! the persisted material back out and hands adapters exactly what they were promised.
use anyhow::Result;
use pcscope_contracts::adapter::{Decoding, EvidenceKind, EvidenceInput, EvidenceRef};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

/// Structure version of the v1 envelopes this receiver accepts. It describes the *source structure*,
/// not the model catalog, so a future envelope shape gets a new value rather than a rewritten row.
pub const PROTOCOL_STRUCTURE_VERSION: &str = "v1";

/// The projection every ingested item is queued for. S07 drains this queue; until then the entries
/// are the durable record that work is outstanding.
pub const OBJECT_PROJECTION: &str = "object_model";

/// Cover both sides of the source/queue join without fetching large payload metadata rows.
/// These additive query indexes apply to fresh and existing schema-8 stores alike.
pub(crate) fn ensure_query_indexes(conn: &Connection) -> Result<()> {
    conn.execute_batch("CREATE INDEX IF NOT EXISTS evidence_source_address ON evidence(source,evidence_id,item_id);
        CREATE INDEX IF NOT EXISTS projection_queue_address_state ON projection_queue(evidence_id,item_id,state)")?;
    let objects:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='objects')",[],|row|row.get(0))?;
    if objects {conn.execute_batch("CREATE INDEX IF NOT EXISTS objects_source_item_scope ON objects(source,id,scope)")?;}
    Ok(())
}

/// Creates the evidence and queue tables. Safe to run repeatedly.
pub fn ensure(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS evidence(
            seq INTEGER PRIMARY KEY AUTOINCREMENT,
            evidence_id TEXT NOT NULL,item_id TEXT NOT NULL,
            kind TEXT NOT NULL,source TEXT NOT NULL,
            structure_version TEXT NOT NULL,source_scope TEXT NOT NULL,
            native_record_id TEXT NOT NULL,
            source_time TEXT,observed_at TEXT NOT NULL,ingested_at TEXT NOT NULL,
            decoding TEXT NOT NULL,payload_ref TEXT NOT NULL,
            pruned_at TEXT,
            UNIQUE(evidence_id,item_id));
         CREATE INDEX IF NOT EXISTS evidence_source ON evidence(source,seq);
         CREATE INDEX IF NOT EXISTS evidence_address ON evidence(kind,evidence_id);

         CREATE TABLE IF NOT EXISTS projection_queue(
            queue_id INTEGER PRIMARY KEY AUTOINCREMENT,
            evidence_id TEXT NOT NULL,item_id TEXT NOT NULL,
            projection TEXT NOT NULL,
            state TEXT NOT NULL,
            attempts INTEGER NOT NULL DEFAULT 0,last_error TEXT,
            enqueued_at TEXT NOT NULL,updated_at TEXT NOT NULL,
            UNIQUE(evidence_id,item_id,projection));
         CREATE INDEX IF NOT EXISTS projection_queue_state ON projection_queue(projection,state,queue_id);",
    )?;
    Ok(())
}

/// Records the evidence for one snapshot's items, and queues each for projection.
///
/// `observed_at` is the collector's own observation time from the envelope; `ingested_at` is when
/// the transaction that stored it committed. The same snapshot replayed inserts nothing and queues
/// nothing a second time.
#[allow(clippy::too_many_arguments)]
pub fn record_snapshot_items(
    tx: &Connection,
    source: &str,
    scope: &str,
    run_id: &str,
    observed_at: &str,
    ingested_at: &str,
    items: &[pcscope_contracts::Item],
    decoding: &Decoding,
) -> Result<usize> {
    let evidence = EvidenceRef::snapshot(source, scope, run_id);
    let decoded = serde_json::to_string(decoding)?;
    let mut queued = 0;
    for item in items {
        let payload_ref = format!("snapshot:{source}|{scope}|{run_id}|{}", item.id);
        queued += insert(
            tx,
            &evidence.id,
            &item.id,
            evidence.kind,
            source,
            scope,
            &item.id,
            None,
            observed_at,
            ingested_at,
            &decoded,
            &payload_ref,
        )?;
    }
    Ok(queued)
}

/// Records the evidence for one event, and queues it for projection.
///
/// The address is `EvidenceRef::event` — source, epoch and event id — so a log that is reset and
/// reuses an id under a new epoch produces a different address rather than colliding with the
/// events that came before. `occurred_at` becomes `source_time`; the batch carries no observation
/// time of its own, so `observed_at` is the receiving time and is stored as what it is.
///
/// `record_seq` is the row this event already occupies in `records`. The two are different things
/// and are kept apart: the address is the source's identity for the event, the sequence is where
/// this database happens to hold it.
pub fn record_event(
    tx: &Connection,
    source: &str,
    epoch: &str,
    event: &pcscope_contracts::Event,
    record_seq: i64,
    observed_at: &str,
    ingested_at: &str,
    decoding: &Decoding,
) -> Result<usize> {
    let evidence = EvidenceRef::event(source, epoch, &event.id);
    insert(
        tx,
        &evidence.id,
        &event.kind,
        evidence.kind,
        source,
        // An event batch states no collection scope: the envelope carries a source, an epoch and a
        // checkpoint, and none of those is one. The epoch was written here, which gave the run log a
        // scope of its own and left it unable to attach to the object it describes. Where a scope is
        // needed it is derived from the machine the collection ran on.
        "",
        &event.id,
        Some(&event.occurred_at),
        observed_at,
        ingested_at,
        &serde_json::to_string(decoding)?,
        &record_payload_ref(record_seq),
    )
}

/// Where a record's material lives. `records.seq` is `AUTOINCREMENT`, so a sequence number is never
/// reused: a pointer cannot come to mean a different record after pruning.
pub fn record_payload_ref(seq: i64) -> String {
    format!("record:{seq}")
}
/// Durable work for a difference proven by one successful complete snapshot of the same raw scope.
#[allow(clippy::too_many_arguments)]
pub(crate) fn record_snapshot_absence(tx:&Connection,source:&str,scope:&str,run:&str,item:&str,seq:i64,observed:&str,ingested:&str)->Result<usize> {
    // Resource samples and collector capability readings have no object lifecycle to retire.
    if pcscope_contracts::adapters::source_gaps::reading_destination(source).is_some(){return Ok(0)}
    let address=format!("snapshot-absence:{}",crate::digest(&serde_json::to_string(&(source,scope,run,item))?));
    insert(tx,&address,item,EvidenceKind::Record,source,scope,item,None,observed,ingested,
        &serde_json::to_string(&decoding_of_stored_material())?,&record_payload_ref(seq))
}
/// Durable addition work, paired with the existing complete-snapshot absence work.
#[allow(clippy::too_many_arguments)]
pub(crate) fn record_snapshot_appearance(tx:&Connection,source:&str,scope:&str,run:&str,item:&str,seq:i64,observed:&str,ingested:&str)->Result<usize> {
    if pcscope_contracts::adapters::source_gaps::reading_destination(source).is_some(){return Ok(0)}
    let address=format!("snapshot-appearance:{}",crate::digest(&serde_json::to_string(&(source,scope,run,item))?));
    insert(tx,&address,item,EvidenceKind::Record,source,scope,item,None,observed,ingested,
        &serde_json::to_string(&decoding_of_stored_material())?,&record_payload_ref(seq))
}
/// Opt-in bounded recovery. The sequence cursor and original records stay independent.
pub(crate) fn repair_snapshot_appearance(tx:&Connection,limit:usize)->Result<Value> {
    anyhow::ensure!((1..=256).contains(&limit),"appearance repair batch must be 1–256 records");
    let cursor:i64=tx.query_row("SELECT value FROM settings WHERE key='projection.appearance_repair_cursor.v1'",[],|row|row.get::<_,String>(0)).optional()?.map(|value|value.parse()).transpose()?.unwrap_or(0);
    let rows:Vec<(i64,String,String,String,String,i64)>= {
        let mut query=tx.prepare("SELECT seq,source,kind,object_id,occurred_at,length(CAST(data AS BLOB)) FROM records WHERE seq>? ORDER BY seq LIMIT ?")?;
        let rows=query.query_map(params![cursor,limit as i64],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?)))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    let(mut next,mut examined,mut queued,mut skipped,mut bytes)=(cursor,0usize,0usize,0usize,0i64);
    for (seq,source,kind,item,at,size) in &rows {
        if kind=="object.first_seen"&&*size<=1048576&&bytes+*size>2097152 {break}
        next=*seq;examined+=1;if kind!="object.first_seen" {continue}
        if *size>1048576 {skipped+=1;continue}
        let text:String=tx.query_row("SELECT data FROM records WHERE seq=?",[seq],|row|row.get(0))?;bytes+=*size;
        let data:Value=match serde_json::from_str(&text){Ok(value)=>value,Err(_)=>{skipped+=1;continue}};
        if data["_snapshot_appearance_v1"]==true {continue}
        let Some(scope)=data["scope"].as_str().filter(|scope|!scope.is_empty())else{skipped+=1;continue};
        if data["basis"]!="snapshot"||data.get("before")!=Some(&Value::Null)||!data["after"].is_object(){skipped+=1;continue}
        // Use one stable legacy address; normal and recovery evidence deduplicate at the action.
        queued+=record_snapshot_appearance(tx,source,scope,&format!("legacy-record-{seq}"),item,*seq,at,at)?;
    }
    tx.execute("INSERT INTO settings(key,value) VALUES('projection.appearance_repair_cursor.v1',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[next.to_string()])?;
    Ok(json!({"examined":examined,"queued":queued,"skipped":skipped,"cursor":next,"material_bytes":bytes,"caught_up":examined==rows.len()&&rows.len()<limit}))
}
/// Opt-in bounded repair for retained legacy differences. Never edits their original material.
pub(crate) fn repair_snapshot_absence(tx:&Connection,limit:usize)->Result<Value> {
    anyhow::ensure!((1..=256).contains(&limit),"absence repair batch must be 1–256 records");
    let cursor:i64=tx.query_row("SELECT value FROM settings WHERE key='projection.absence_repair_cursor.v1'",[],|row|row.get::<_,String>(0)).optional()?.map(|value|value.parse()).transpose()?.unwrap_or(0);
    let rows:Vec<(i64,String,String,Option<String>,String,i64)>= {
        let mut query=tx.prepare("SELECT seq,source,kind,object_id,occurred_at,length(CAST(data AS BLOB)) FROM records WHERE seq>? ORDER BY seq LIMIT ?")?;
        let rows=query.query_map(params![cursor,limit as i64],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?)))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    let(mut next,mut examined,mut queued,mut skipped,mut bytes)=(cursor,0usize,0usize,0usize,0i64);
    for (seq,source,kind,item,at,size) in &rows {
        if kind=="object.no_longer_observed"&&*size<=1048576&&bytes+*size>2097152 {break}
        next=*seq;examined+=1;
        if kind!="object.no_longer_observed" {continue}
        if *size>1048576||item.is_none(){skipped+=1;continue}
        let text:String=tx.query_row("SELECT data FROM records WHERE seq=?",[seq],|row|row.get(0))?;bytes+=*size;
        let data:Value=match serde_json::from_str(&text){Ok(value)=>value,Err(_)=>{skipped+=1;continue}};
        // New ingestion already queues this difference atomically. Legacy repair uses a stable
        // address and the unique evidence key, so neither path needs an evidence-table scan.
        if data["_snapshot_absence_v1"]==true {continue}
        let scope=match data["scope"].as_str().filter(|scope|!scope.is_empty()){Some(scope)=>scope,None=>{skipped+=1;continue}};
        if data["basis"]!="snapshot"||data.get("after")!=Some(&Value::Null)||!data["before"].is_object(){skipped+=1;continue}
        queued+=record_snapshot_absence(tx,source,scope,&format!("legacy-record-{seq}"),item.as_ref().unwrap(),*seq,at,at)?;
    }
    tx.execute("INSERT INTO settings(key,value) VALUES('projection.absence_repair_cursor.v1',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[next.to_string()])?;
    Ok(json!({"examined":examined,"queued":queued,"skipped":skipped,"cursor":next,"material_bytes":bytes,"caught_up":examined==rows.len()&&rows.len()<limit}))
}

#[allow(clippy::too_many_arguments)]
fn insert(
    tx: &Connection,
    evidence_id: &str,
    item_id: &str,
    kind: EvidenceKind,
    source: &str,
    source_scope: &str,
    native_record_id: &str,
    source_time: Option<&str>,
    observed_at: &str,
    ingested_at: &str,
    decoding: &str,
    payload_ref: &str,
) -> Result<usize> {
    let kind = match kind {
        EvidenceKind::Record => "record",
        EvidenceKind::Snapshot => "snapshot",
        EvidenceKind::Event => "event",
    };
    // Evidence and its work item are written together. A conflict means this exact material was
    // already ingested, so it is neither stored nor queued again — replaying a batch is a no-op all
    // the way down, not just at the raw table.
    let stored = tx.execute(
        "INSERT INTO evidence(evidence_id,item_id,kind,source,structure_version,source_scope,native_record_id,source_time,observed_at,ingested_at,decoding,payload_ref)
         VALUES(?,?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(evidence_id,item_id) DO NOTHING",
        params![evidence_id, item_id, kind, source, PROTOCOL_STRUCTURE_VERSION, source_scope, native_record_id, source_time, observed_at, ingested_at, decoding, payload_ref],
    )?;
    if stored == 0 {
        return Ok(0);
    }
    tx.execute(
        "INSERT INTO projection_queue(evidence_id,item_id,projection,state,enqueued_at,updated_at)
         VALUES(?,?,?,'pending',?,?) ON CONFLICT(evidence_id,item_id,projection) DO NOTHING",
        params![evidence_id, item_id, OBJECT_PROJECTION, ingested_at, ingested_at],
    )?;
    Ok(1)
}

/// The material an ingest stored that the source itself declared it could not read.
///
/// The v1 envelopes carry no channel for per-item read problems: a collector states what it could
/// not read through a `gap` message, and anything finer than that the protocol cannot express yet.
/// This is recorded rather than guessed at, and stays empty until the protocol carries it.
pub fn decoding_of_stored_material() -> Decoding {
    Decoding { available: true, ..Default::default() }
}

/// Reads persisted evidence back out as adapter inputs: the store-backed form of
/// `reference::evidence_from`.
///
/// `after_seq` makes the read resumable, so a projection can work through evidence in the order it
/// arrived without holding the whole table in memory.
pub fn inputs(conn: &Connection, after_seq: i64, limit: usize) -> Result<Vec<EvidenceInput>> {
    let mut statement = conn.prepare(
        "SELECT seq,evidence_id,item_id,kind,source,structure_version,source_scope,native_record_id,
                source_time,observed_at,ingested_at,decoding,payload_ref
         FROM evidence WHERE seq>? ORDER BY seq LIMIT ?",
    )?;
    let rows = statement.query_map(params![after_seq, limit as i64], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            StoredEvidence {
                evidence_id: row.get(1)?,
                item_id: row.get(2)?,
                kind: row.get(3)?,
                source: row.get(4)?,
                structure_version: row.get(5)?,
                source_scope: row.get(6)?,
                native_record_id: row.get(7)?,
                source_time: row.get(8)?,
                observed_at: row.get(9)?,
                ingested_at: row.get(10)?,
                decoding: row.get(11)?,
                payload_ref: row.get(12)?,
            },
        ))
    })?;
    let stored = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    to_inputs(conn, stored)
}

/// One piece of evidence, read back by its address. A projection working through the queue asks
/// for exactly the item it was handed rather than reading the whole table.
pub fn input_at(conn: &Connection, evidence_id: &str, item_id: &str) -> Result<Option<EvidenceInput>> {
    input_at_cached(conn, evidence_id, item_id, &mut SnapshotCache::default())
}

/// Keep at most one snapshot, and only for a single projection transaction. No stale cache
/// survives a rollback, a retention operation or the next batch.
#[derive(Default)]
pub(crate) struct SnapshotCache {
    loaded: Option<(String, std::collections::BTreeMap<String, Value>)>,
    #[cfg(test)]
    loads: usize,
}

pub(crate) fn input_at_cached(conn: &Connection, evidence_id: &str, item_id: &str, cache: &mut SnapshotCache) -> Result<Option<EvidenceInput>> {
    let stored: Option<(i64, StoredEvidence)> = conn
        .query_row(
            "SELECT seq,evidence_id,item_id,kind,source,structure_version,source_scope,native_record_id,
                    source_time,observed_at,ingested_at,decoding,payload_ref
             FROM evidence WHERE evidence_id=? AND item_id=?",
            params![evidence_id, item_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    StoredEvidence {
                        evidence_id: row.get(1)?,
                        item_id: row.get(2)?,
                        kind: row.get(3)?,
                        source: row.get(4)?,
                        structure_version: row.get(5)?,
                        source_scope: row.get(6)?,
                        native_record_id: row.get(7)?,
                        source_time: row.get(8)?,
                        observed_at: row.get(9)?,
                        ingested_at: row.get(10)?,
                        decoding: row.get(11)?,
                        payload_ref: row.get(12)?,
                    },
                ))
            },
        )
        .optional()?;
    match stored {
        Some(row) => Ok(to_inputs_cached(conn, vec![row], cache)?.pop()),
        None => Ok(None),
    }
}

fn to_inputs(conn: &Connection, stored: Vec<(i64, StoredEvidence)>) -> Result<Vec<EvidenceInput>> {
    to_inputs_cached(conn, stored, &mut SnapshotCache::default())
}

fn to_inputs_cached(conn: &Connection, stored: Vec<(i64, StoredEvidence)>, cache: &mut SnapshotCache) -> Result<Vec<EvidenceInput>> {
    let mut out = Vec::with_capacity(stored.len());
    // A snapshot payload holds every item, so consecutive items of the same run reuse one parse
    // instead of re-reading the whole inventory per item.
    for (_, row) in stored {
        let payload = if row.kind == "snapshot" {
            let key = row.snapshot_reference();
            if cache.loaded.as_ref().map(|(loaded, _)| loaded) != Some(&key) {
                let whole = read_snapshot_payload(conn, &key)?;
                let mut items = std::collections::BTreeMap::new();
                if let Some(rows) = whole.get("items").and_then(Value::as_array) {
                    for item in rows {
                        if let Some(id) = item.get("id").and_then(Value::as_str) {
                            // Match the previous first-match behavior even for malformed duplicates.
                            items.entry(id.to_owned()).or_insert_with(||item.get("data").cloned().unwrap_or(Value::Null));
                        }
                    }
                }
                cache.loaded = Some((key, items));
                #[cfg(test)] { cache.loads += 1; }
            }
            cache.loaded.as_ref()
                .and_then(|(_, items)|items.get(&row.item_id))
                .cloned()
                .unwrap_or(Value::Null)
        } else {
            read_record_payload(conn, &row.payload_ref)?
        };
        out.push(row.into_input(payload)?);
    }
    Ok(out)
}

struct StoredEvidence {
    evidence_id: String,
    item_id: String,
    kind: String,
    source: String,
    structure_version: String,
    source_scope: String,
    native_record_id: String,
    source_time: Option<String>,
    observed_at: String,
    ingested_at: String,
    decoding: String,
    payload_ref: String,
}

impl StoredEvidence {
    fn snapshot_reference(&self) -> String {
        self.payload_ref.split('|').take(3).collect::<Vec<_>>().join("|")
    }
    fn into_input(self, payload: Value) -> Result<EvidenceInput> {
        let kind = match self.kind.as_str() {
            "snapshot" => EvidenceKind::Snapshot,
            "event" => EvidenceKind::Event,
            _ => EvidenceKind::Record,
        };
        Ok(EvidenceInput {
            source: self.source,
            item_id: self.item_id,
            structure_version: self.structure_version,
            source_scope: self.source_scope,
            evidence: EvidenceRef { kind, id: self.evidence_id },
            native_record_id: self.native_record_id,
            source_time: self.source_time,
            observed_at: self.observed_at,
            ingested_at: self.ingested_at,
            decoding: serde_json::from_str(&self.decoding)?,
            payload,
        })
    }
}

/// Reads a snapshot verbatim, addressed by source, scope and run.
fn read_snapshot_payload(conn: &Connection, key: &str) -> Result<Value> {
    let rest = key.strip_prefix("snapshot:").unwrap_or(key);
    let mut parts = rest.splitn(3, '|');
    let (source, scope, run) = (parts.next().unwrap_or_default(), parts.next().unwrap_or_default(), parts.next().unwrap_or_default());
    let payload: Option<String> = conn
        .query_row("SELECT payload FROM snapshots WHERE source=? AND scope=? AND run_id=?", params![source, scope, run], |r| r.get(0))
        .optional()?;
    match payload {
        Some(text) => Ok(serde_json::from_str(&text)?),
        // The raw material is gone — pruned by the retention sweep, or never committed. The input
        // is still produced with a null payload so the caller sees "the evidence is not here"
        // rather than a silently missing row.
        None => Ok(Value::Null),
    }
}

/// Reads one event's material out of `records`.
fn read_record_payload(conn: &Connection, payload_ref: &str) -> Result<Value> {
    let seq: i64 = payload_ref.strip_prefix("record:").unwrap_or_default().parse().unwrap_or(0);
    let data: Option<String> = conn.query_row("SELECT data FROM records WHERE seq=?", params![seq], |r| r.get(0)).optional()?;
    match data {
        Some(text) => Ok(serde_json::from_str(&text)?),
        None => Ok(Value::Null),
    }
}

/// Marks the evidence whose material a retention unit holds, and the references that cite it.
///
/// A fact or an action keeps its own value and observation time when its evidence is pruned — what
/// it loses is the ability to show the material behind it, and that has to be a stated state rather
/// than a pointer into a row that is no longer there. Event evidence is addressed by source, epoch
/// and event id rather than by record sequence, so marking it needs the payload reference rather
/// than the address; matching on the address alone would leave those references dangling.
pub fn mark_pruned_for_unit(conn: &Connection, unit: i64, pruned_at: &str) -> Result<usize> {
    let addresses: Vec<(String, String, String)> = {
        let mut statement = conn.prepare(
            "SELECT evidence_id,item_id,kind FROM evidence
             WHERE pruned_at IS NULL AND (
                payload_ref IN (SELECT 'record:'||seq FROM records WHERE unit_id=?1)
                OR EXISTS (SELECT 1 FROM snapshots s WHERE s.unit_id=?1
                           AND evidence.payload_ref LIKE 'snapshot:'||s.source||'|'||s.scope||'|'||s.run_id||'|%'))",
        )?;
        let rows = statement.query_map(params![unit], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    for (evidence_id, item_id, kind) in &addresses {
        conn.execute("UPDATE evidence SET pruned_at=? WHERE evidence_id=? AND item_id=? AND pruned_at IS NULL", params![pruned_at, evidence_id, item_id])?;
        conn.execute(
            "UPDATE evidence_refs SET pruned_at=? WHERE pruned_at IS NULL AND evidence_kind=? AND evidence_id=?",
            params![pruned_at, kind, evidence_id],
        )?;
    }
    Ok(addresses.len())
}

/// Queue depth and state counts, for the collection-health view.
pub fn queue_summary(conn: &Connection, projection: &str) -> Result<Value> {
    let mut statement = conn.prepare("SELECT state,count(*) FROM projection_queue WHERE projection=? GROUP BY state")?;
    let rows = statement.query_map(params![projection], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))?;
    let mut counts = serde_json::Map::new();
    let mut total = 0;
    for row in rows {
        let (state, count) = row?;
        total += count;
        counts.insert(state, Value::from(count));
    }
    Ok(serde_json::json!({"projection": projection, "total": total, "by_state": counts}))
}

#[cfg(test)]
mod cache_tests {
    use super::*;
    #[test]
    fn legacy_absence_repair_is_bounded_resumable_and_preserves_original_material() {
        let store = crate::Store::open(":memory:").unwrap();
        let original = json!({"scope":"machine:M","basis":"snapshot","before":{"name":"device"},"after":null}).to_string();
        for (kind, material) in [("unrelated", "{}".to_string()),
            ("object.no_longer_observed", original.clone()),
            ("object.no_longer_observed", "{}".to_string()),
            ("object.no_longer_observed", json!({"scope":"machine:M","basis":"snapshot","before":{},"after":null,"_snapshot_absence_v1":true}).to_string())] {
            store.reader().execute("INSERT INTO records(source,kind,object_id,occurred_at,data) VALUES('pcscope.hardware.devices',?,'device','2026-10-14T00:00:00Z',?)", params![kind,material]).unwrap();
        }
        assert!(store.repair_snapshot_absence(0).is_err());
        assert!(store.repair_snapshot_absence(257).is_err());
        let first=store.repair_snapshot_absence(2).unwrap();
        assert_eq!(first["examined"],2);assert_eq!(first["queued"],1);assert_eq!(first["cursor"],2);
        let second=store.repair_snapshot_absence(2).unwrap();
        assert_eq!(second["queued"],0);assert_eq!(second["skipped"],1);assert_eq!(second["cursor"],4);
        assert_eq!(store.repair_snapshot_absence(2).unwrap()["caught_up"],true);
        let retained:String=store.reader().query_row("SELECT data FROM records WHERE seq=2",[],|row|row.get(0)).unwrap();
        assert_eq!(retained,original);
        store.reader().execute("DELETE FROM settings WHERE key='projection.absence_repair_cursor.v1'",[]).unwrap();
        assert_eq!(store.repair_snapshot_absence(4).unwrap()["queued"],0);
        let count:i64=store.reader().query_row("SELECT count(*) FROM evidence WHERE evidence_id LIKE 'snapshot-absence:%'",[],|row|row.get(0)).unwrap();
        assert_eq!(count,1);
    }
    #[test]
    fn coverage_join_has_a_covering_source_address_index() {
        let store = crate::Store::open(":memory:").unwrap();
        let columns = store.reader().prepare("PRAGMA index_info(evidence_source_address)").unwrap()
            .query_map([],|row|row.get::<_,String>(2)).unwrap()
            .collect::<rusqlite::Result<Vec<_>>>().unwrap();
        assert_eq!(columns,["source","evidence_id","item_id"]);
        let columns = store.reader().prepare("PRAGMA index_info(projection_queue_address_state)").unwrap()
            .query_map([],|row|row.get::<_,String>(2)).unwrap()
            .collect::<rusqlite::Result<Vec<_>>>().unwrap();
        assert_eq!(columns,["evidence_id","item_id","state"]);
    }
    #[test]
    fn repeated_items_parse_once_and_run_changes_never_reuse_old_payloads() {
        let mut store = crate::Store::open(":memory:").unwrap();
        for run in ["one", "two"] {
            let message = serde_json::json!({"protocol_version":1,"request_id":run,"type":"snapshot",
                "source":"pcscope.startup.tasks","scope":"machine:M","run_id":run,
                "observed_at":"2026-09-12T00:00:00Z","complete":true,
                "items":[{"id":"a","data":{"path":"a","name":run}},{"id":"b","data":{"path":"b","name":run}}]});
            store.ingest(&pcscope_contracts::parse(&message.to_string()).unwrap()).unwrap();
        }
        let mut cache = SnapshotCache::default();
        for (run, expected_loads) in [("one",1),("two",2),("one",3)] {
            let address = EvidenceRef::snapshot("pcscope.startup.tasks","machine:M",run);
            for item in ["a", "b"] {
                let cached = input_at_cached(store.reader(), &address.id, item, &mut cache).unwrap().unwrap();
                let uncached = input_at(store.reader(), &address.id, item).unwrap().unwrap();
                assert_eq!(cached.payload, uncached.payload);
                assert_eq!(cached.payload["name"],run);
                assert_eq!(cache.loads,expected_loads);
            }
        }
        assert!(input_at_cached(store.reader(),"missing","a",&mut cache).unwrap().is_none());
    }
}
