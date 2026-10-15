//! S28: reconciling the model against the material it was built from.
//!
//! The plan is explicit that matching object counts is not reconciliation, so nothing here reports
//! a total and calls it agreement. Every section answers a question about **individual inputs**:
//!
//!   * which retained material has no processing status at all — neither projected nor failed;
//!   * which inputs failed, grouped by source, with the reason each failed;
//!   * whether the current value of every sampled field is backed by evidence that still exists;
//!   * whether any identity was merged that should not have been, and which instances are
//!     provisional so a reader knows where the model is unsure of itself;
//!   * whether everything that arrived has been dealt with, by comparing the evidence watermark with
//!     the queue's.
//!
//! What it cannot decide, it reports rather than settles. An instance whose facts come from several
//! sources is the case where a wrong merge would hide, so those are listed for a person to sample —
//! the tool can see that two sources meet on one instance, and cannot see from here whether they
//! should have.
use anyhow::Result;
use rusqlite::Connection;
use serde_json::{json, Value};

/// The whole reconciliation, as one report.
pub fn report(conn: &Connection) -> Result<Value> {
    Ok(json!({
        "inputs": inputs(conn)?,
        "failures": failures(conn)?,
        "unprocessed": unprocessed(conn)?,
        "field_trace": field_trace(conn)?,
        "identity": identity(conn)?,
        "watermarks": watermarks(conn)?,
    }))
}

/// Every retained input and what became of it.
///
/// The status is per input rather than per total: an input with no queue entry is a different
/// finding from one that failed, and both are different from one that was projected.
fn inputs(conn: &Connection) -> Result<Value> {
    let mut statement = conn.prepare(
        "SELECT e.source,
                count(*) AS evidence,
                sum(CASE WHEN q.state='done' THEN 1 ELSE 0 END) AS projected,
                sum(CASE WHEN q.state='failed' THEN 1 ELSE 0 END) AS failed,
                sum(CASE WHEN q.state='pending' THEN 1 ELSE 0 END) AS pending,
                sum(CASE WHEN q.queue_id IS NULL THEN 1 ELSE 0 END) AS unqueued
         FROM evidence e
         LEFT JOIN projection_queue q ON q.evidence_id=e.evidence_id AND q.item_id=e.item_id
         GROUP BY e.source ORDER BY e.source",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(json!({
            "source": row.get::<_, String>(0)?,
            "evidence": row.get::<_, i64>(1)?,
            "projected": row.get::<_, i64>(2)?,
            "failed": row.get::<_, i64>(3)?,
            "pending": row.get::<_, i64>(4)?,
            "unqueued": row.get::<_, i64>(5)?,
        }))
    })?;
    Ok(Value::Array(rows.collect::<rusqlite::Result<Vec<_>>>()?))
}

/// Every parse failure, with the reason it failed.
///
/// A failure is a finding, not a total: the plan asks whether each one has been fixed or is
/// unrecoverable input, and that question is answered per source and per reason.
fn failures(conn: &Connection) -> Result<Value> {
    let mut statement = conn.prepare(
        "SELECT e.source, coalesce(q.last_error,'（未记录原因）') AS reason, count(*) AS count
         FROM projection_queue q JOIN evidence e ON e.evidence_id=q.evidence_id AND e.item_id=q.item_id
         WHERE q.state='failed'
         GROUP BY e.source, reason ORDER BY count DESC, e.source",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(json!({
            "source": row.get::<_, String>(0)?,
            "reason": row.get::<_, String>(1)?,
            "count": row.get::<_, i64>(2)?,
        }))
    })?;
    Ok(Value::Array(rows.collect::<rusqlite::Result<Vec<_>>>()?))
}

/// Material that has no processing status at all.
///
/// This is the category the plan's first acceptance condition is about: an input nothing has decided
/// about is not the same as one that succeeded, and a report that counted only successes would show
/// it as absent rather than as outstanding.
fn unprocessed(conn: &Connection) -> Result<Value> {
    let rows: i64 = conn.query_row(
        "SELECT count(*) FROM evidence e
         LEFT JOIN projection_queue q ON q.evidence_id=e.evidence_id AND q.item_id=e.item_id
         WHERE q.queue_id IS NULL",
        [],
        |r| r.get(0),
    )?;
    let sample: Vec<Value> = {
        let mut statement = conn.prepare(
            "SELECT e.evidence_id, e.source, e.ingested_at FROM evidence e
             LEFT JOIN projection_queue q ON q.evidence_id=e.evidence_id AND q.item_id=e.item_id
             WHERE q.queue_id IS NULL ORDER BY e.seq LIMIT 20",
        )?;
        let rows = statement.query_map([], |row| Ok(json!({"evidence_id": row.get::<_, String>(0)?, "source": row.get::<_, String>(1)?, "ingested_at": row.get::<_, String>(2)?}))) ?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    Ok(json!({"count": rows, "first": sample}))
}

/// Whether the current value of each sampled field is backed by evidence that still exists.
///
/// A field whose evidence has been pruned is not a mismatch: R04 keeps the conclusion and its
/// observation time when the material goes, and marks the reference as pruned. What would be a
/// finding is a field citing evidence that is neither present nor marked pruned — a reference into
/// nothing.
fn field_trace(conn: &Connection) -> Result<Value> {
    let cited: i64 = conn.query_row(
        "SELECT count(DISTINCT r.evidence_id) FROM evidence_refs r WHERE r.subject_kind='observation'",
        [],
        |r| r.get(0),
    )?;
    let dangling: i64 = conn.query_row(
        "SELECT count(*) FROM evidence_refs r
         WHERE r.subject_kind='observation' AND r.pruned_at IS NULL
           AND NOT EXISTS (SELECT 1 FROM evidence e WHERE e.evidence_id=r.evidence_id)",
        [],
        |r| r.get(0),
    )?;
    let facts: i64 = conn.query_row("SELECT count(*) FROM object_facts", [], |r| r.get(0))?;
    let without_reference: i64 = conn.query_row(
        "SELECT count(*) FROM object_facts f WHERE f.value_status IN ('known','conflicting','masked')
           AND NOT EXISTS (SELECT 1 FROM evidence_refs r WHERE r.subject_kind='observation' AND r.subject_id=CAST(f.instance_id AS TEXT))",
        [],
        |r| r.get(0),
    )?;
    // A fact citing evidence observed later than the fact itself means a newer observation of that
    // field exists and the current value is older than it — the current state lagging the evidence,
    // which the plan's second acceptance condition is about.
    let behind: i64 = conn.query_row(
        "SELECT count(*) FROM object_facts f
         WHERE f.value_status IN ('known','conflicting','masked')
           AND EXISTS (
             SELECT 1 FROM evidence_refs r JOIN evidence e ON e.evidence_id=r.evidence_id
             WHERE r.subject_kind='observation' AND r.subject_id=CAST(f.instance_id AS TEXT)
               AND e.observed_at > f.observed_at)",
        [],
        |r| r.get(0),
    )?;
    Ok(json!({
        "cited_evidence": cited,
        // A current value older than evidence the model already holds is a mismatch, not a lag.
        "facts_behind_their_evidence": behind,
        "facts": facts,
        // A fact with a value and no evidence at all is the finding this section is for.
        "facts_without_evidence": without_reference,
        // A reference to evidence that is neither present nor marked pruned is a dangling pointer.
        "dangling_references": dangling,
    }))
}

/// Where the model is unsure of an identity, and where a wrong merge could hide.
fn identity(conn: &Connection) -> Result<Value> {
    let mut statement = conn.prepare("SELECT identity_status, count(*) FROM object_instances GROUP BY identity_status")?;
    let rows = statement.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))?;
    let mut by_status = serde_json::Map::new();
    for row in rows {
        let (status, count) = row?;
        by_status.insert(status, Value::from(count));
    }
    // An instance whose facts arrive from more than one source is where sources were merged. The tool
    // can see that they met; whether they should have is a question about the model, so they are
    // listed for a person to sample rather than judged here.
    let multi_source: Vec<Value> = {
        let mut statement = conn.prepare(
            "SELECT b.instance_id, count(DISTINCT b.source) AS sources, group_concat(DISTINCT b.source) AS which
             FROM source_bindings b WHERE b.valid_to IS NULL
             GROUP BY b.instance_id HAVING sources > 1 ORDER BY sources DESC, b.instance_id LIMIT 50",
        )?;
        let rows = statement.query_map([], |row| Ok(json!({"instance_id": row.get::<_, i64>(0)?, "sources": row.get::<_, i64>(1)?, "bound_sources": row.get::<_, String>(2)?})))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    // A key carrying the missing-marker is provisional by construction: the identity layer keeps a
    // placeholder so two incomplete identities cannot collide, and this counts them.
    let provisional_keys: i64 = conn.query_row("SELECT count(*) FROM object_instances WHERE native_key LIKE '%?%'", [], |r| r.get(0))?;
    Ok(json!({
        "by_status": by_status,
        "multi_source_instances": multi_source.len(),
        "multi_source_sample": multi_source,
        "provisional_keys": provisional_keys,
    }))
}

/// Whether everything that arrived has been dealt with.
///
/// The two watermark values are what the plan's last acceptance condition turns on: work queued by
/// material that arrived during the migration has to be finished before the run is called complete,
/// and this is the comparison that says whether it was.
fn watermarks(conn: &Connection) -> Result<Value> {
    let evidence: i64 = conn.query_row("SELECT coalesce(max(seq),0) FROM evidence", [], |r| r.get(0))?;
    let queued: i64 = conn.query_row(
        "SELECT coalesce(max(e.seq),0) FROM projection_queue q JOIN evidence e ON e.evidence_id=q.evidence_id AND e.item_id=q.item_id",
        [],
        |r| r.get(0),
    )?;
    let pending: i64 = conn.query_row("SELECT count(*) FROM projection_queue WHERE state='pending'", [], |r| r.get(0))?;
    let failed: i64 = conn.query_row("SELECT count(*) FROM projection_queue WHERE state='failed'", [], |r| r.get(0))?;
    Ok(json!({
        "evidence_watermark": evidence,
        "queued_watermark": queued,
        // Everything that arrived is either dealt with or outstanding — there is no third state, and
        // an input that is neither is what `unprocessed` reports.
        "everything_queued": evidence == queued,
        "pending": pending,
        "failed": failed,
        "settled": pending == 0,
    }))
}

/// The one-line verdict, for a caller that wants to know whether to look further.
pub fn verdict(report: &Value) -> &'static str {
    let inputs = report["inputs"].as_array().cloned().unwrap_or_default();
    let outstanding = inputs.iter().any(|row| row["unqueued"].as_i64().unwrap_or(0) > 0 || row["pending"].as_i64().unwrap_or(0) > 0);
    let dangling = report["field_trace"]["dangling_references"].as_i64().unwrap_or(0);
    let orphan_facts = report["field_trace"]["facts_without_evidence"].as_i64().unwrap_or(0);
    let behind = report["field_trace"]["facts_behind_their_evidence"].as_i64().unwrap_or(0);
    if outstanding || dangling > 0 || orphan_facts > 0 || behind > 0 {
        return "incomplete";
    }
    let failures = report["watermarks"]["failed"].as_i64().unwrap_or(0);
    if failures > 0 {
        // Failures are reported rather than repaired: the plan asks whether each is fixable, and
        // that is a question about an adapter or about the input, not one this can answer.
        return "reconciled_with_failures";
    }
    "reconciled"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Store;

    const TASKS: &str = "pcscope.startup.tasks";
    const SCOPE: &str = "machine:M/install:I/schema:1/context:/shard:0";

    fn snapshot(run: &str, names: &[&str]) -> pcscope_contracts::Envelope {
        let items: Vec<Value> = names.iter().map(|name| json!({"id": format!("\\A\\{name}"), "data": {"path": format!("\\A\\{name}"), "name": name}})).collect();
        pcscope_contracts::parse(&json!({
            "protocol_version": 1, "request_id": run, "type": "snapshot", "source": TASKS,
            "scope": SCOPE, "run_id": run, "observed_at": "2026-09-12T00:00:00Z", "complete": true, "items": items
        }).to_string())
        .unwrap()
    }

    #[test]
    fn every_input_gets_a_status_and_the_verdict_says_when_one_has_none() {
        let mut store = Store::open(":memory:").unwrap();
        store.ingest(&snapshot("run-1", &["task-00"])).unwrap();
        // Nothing has been projected yet, so the input is queued and outstanding.
        let before = report(&store.conn).unwrap();
        assert_eq!(before["inputs"][0]["pending"], 1);
        assert_eq!(verdict(&before), "incomplete");

        store.project_pending("M", "I", 100).unwrap();
        let after = report(&store.conn).unwrap();
        assert_eq!(after["inputs"][0]["projected"], 1);
        assert_eq!(after["inputs"][0]["unqueued"], 0);
        assert_eq!(verdict(&after), "reconciled");
    }

    #[test]
    fn a_field_with_a_value_and_no_evidence_is_a_finding() {
        let mut store = Store::open(":memory:").unwrap();
        store.ingest(&snapshot("run-1", &["task-00"])).unwrap();
        store.project_pending("M", "I", 100).unwrap();
        let clean = report(&store.conn).unwrap();
        assert_eq!(clean["field_trace"]["facts_without_evidence"], 0);
        assert_eq!(clean["field_trace"]["dangling_references"], 0);
        assert_eq!(clean["field_trace"]["facts_behind_their_evidence"], 0, "a current value must not be older than evidence the model holds");

        // A value the model states with nothing behind it is precisely what the report exists to
        // find, and the counts above would not have caught it.
        store.conn.execute("DELETE FROM evidence_refs", []).unwrap();
        let broken = report(&store.conn).unwrap();
        assert!(broken["field_trace"]["facts_without_evidence"].as_i64().unwrap() > 0);
        assert_eq!(verdict(&broken), "incomplete");
    }

    #[test]
    fn failures_are_reported_with_the_reason_they_failed() {
        let mut store = Store::open(":memory:").unwrap();
        // A source with no adapter: its material is retained and its work item fails with a reason.
        let unknown = pcscope_contracts::parse(&json!({
            "protocol_version": 1, "request_id": "run-x", "type": "snapshot", "source": "pcscope.unconverted.thing",
            "scope": SCOPE, "run_id": "run-x", "observed_at": "2026-09-12T00:00:00Z", "complete": true,
            "items": [{"id": "x", "data": {}}]
        }).to_string())
        .unwrap();
        store.ingest(&unknown).unwrap();
        store.project_pending("M", "I", 100).unwrap();

        let report = report(&store.conn).unwrap();
        let failures = report["failures"].as_array().unwrap();
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0]["source"], "pcscope.unconverted.thing");
        assert!(failures[0]["reason"].as_str().unwrap().contains("no adapter"));
        // A failure does not make the run incomplete — everything arrived and was decided — but it
        // is not "reconciled" either.
        assert_eq!(verdict(&report), "reconciled_with_failures");
    }

    #[test]
    fn the_watermarks_say_whether_everything_that_arrived_was_dealt_with() {
        let mut store = Store::open(":memory:").unwrap();
        store.ingest(&snapshot("run-1", &["task-00"])).unwrap();
        store.project_pending("M", "I", 100).unwrap();
        let settled = report(&store.conn).unwrap();
        assert_eq!(settled["watermarks"]["everything_queued"], true);
        assert_eq!(settled["watermarks"]["settled"], true);

        // Material that arrives after the last projection has a queue entry and no decision yet.
        store.ingest(&snapshot("run-2", &["task-01"])).unwrap();
        let outstanding = report(&store.conn).unwrap();
        assert_eq!(outstanding["watermarks"]["settled"], false);
        assert_eq!(verdict(&outstanding), "incomplete");
    }

    #[test]
    fn an_instance_bound_to_two_sources_is_listed_for_a_person_to_sample() {
        let mut store = Store::open(":memory:").unwrap();
        let details = pcscope_contracts::parse(&json!({
            "protocol_version": 1, "request_id": "run-d", "type": "snapshot", "source": "pcscope.startup.task-details",
            "scope": SCOPE, "run_id": "run-d", "observed_at": "2026-09-12T00:00:00Z", "complete": true,
            "items": [{"id": "\\A\\task-00", "data": {"path": "\\A\\task-00", "name": "task-00"}}]
        }).to_string())
        .unwrap();
        store.ingest(&snapshot("run-1", &["task-00"])).unwrap();
        store.ingest(&details).unwrap();
        store.project_pending("M", "I", 100).unwrap();

        let report = report(&store.conn).unwrap();
        // The two sources met on one instance, which is what converged identity looks like — and is
        // also where a wrong merge would look the same. The tool lists it and does not judge it.
        assert_eq!(report["identity"]["multi_source_instances"], 1);
        assert!(report["identity"]["multi_source_sample"][0]["bound_sources"].as_str().unwrap().contains(TASKS));
        assert_eq!(report["identity"]["by_status"]["confirmed"], 1);
    }
}
