//! S30: the object model's own cost, and what the capacity modes actually do.
//!
//! The S02 baseline measures the old store's queries. This measures the model that replaced it —
//! ingestion, projection and the size the result takes — and then runs the four capacity
//! behaviours the plan names, because each of them is an acceptance condition rather than a
//! measurement:
//!
//!   * **steady state** — repeated collection of the same material must not grow the store. A model
//!     that appends something on every unchanged round is one that fills a disk while the data
//!     stands still;
//!   * **rolling cleanup** — a quota is honoured, the oldest history goes, and what is left is
//!     explainable: every reference to pruned material says so;
//!   * **unlimited** — nothing is deleted, on any schedule;
//!   * **insufficient space** — a store that cannot hold what is necessary says so, rather than
//!     reporting a successful collection.
//!
//! usage: model-capacity [output-path] [rounds]
use anyhow::Result;
use pcscope_storage::Store;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Instant;

const SOURCE: &str = "pcscope.startup.tasks";
const SCOPE: &str = "machine:M/install:I/schema:1/context:/shard:0";

fn snapshot(run: &str, count: usize, name: &str) -> pcscope_contracts::Envelope {
    let items: Vec<Value> = (0..count).map(|index| json!({"id": format!("\\A\\task-{index:03}"), "data": {"path": format!("\\A\\task-{index:03}"), "name": format!("{name}-{index:03}")}})).collect();
    pcscope_contracts::parse(&json!({
        "protocol_version": 1, "request_id": run, "type": "snapshot", "source": SOURCE,
        "scope": SCOPE, "run_id": run, "observed_at": "2026-09-12T00:00:00Z", "complete": true, "items": items
    }).to_string())
    .unwrap()
}

fn count(store: &Store, sql: &str) -> i64 {
    store.reader().query_row(sql, [], |r| r.get(0)).unwrap()
}

fn bytes(store: &Store) -> i64 {
    store.reader().query_row("SELECT page_count * page_size FROM pragma_page_count(), pragma_page_size()", [], |r| r.get(0)).unwrap()
}

fn components(path: &PathBuf) -> Result<Value> {
    let mut sizes = serde_json::Map::new();
    for suffix in ["", "-wal", "-shm"] {
        let mut name = path.as_os_str().to_os_string();
        name.push(suffix);
        let size = std::fs::metadata(PathBuf::from(name)).map(|meta| meta.len()).unwrap_or(0);
        sizes.insert(if suffix.is_empty() { "database".into() } else { suffix.trim_start_matches('-').to_string() }, Value::from(size));
    }
    Ok(Value::Object(sizes))
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("pcscope-capacity-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("history.db")
}

fn remove(path: &PathBuf) {
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

/// Repeated collection of the same material: the store must reach a size and stay there.
fn steady_state(rounds: usize) -> Result<Value> {
    let path = scratch("steady");
    let mut store = Store::open(&path)?;
    store.ingest(&snapshot("run-1", 200, "task"))?;
    store.project_pending("M", "I", 10_000)?;
    let settled = bytes(&store);
    let counts = json!({
        "evidence": count(&store, "SELECT count(*) FROM evidence"),
        "facts": count(&store, "SELECT count(*) FROM object_facts"),
        "actions": count(&store, "SELECT count(*) FROM object_actions"),
        "queue": count(&store, "SELECT count(*) FROM projection_queue"),
    });
    let start = Instant::now();
    for round in 0..rounds {
        // The same items again, unchanged. Only the run id differs, as a new collection round would.
        store.ingest(&snapshot(&format!("run-steady-{round}"), 200, "task"))?;
        store.project_pending("M", "I", 10_000)?;
    }
    let elapsed = start.elapsed().as_millis() as u64;
    let after = bytes(&store);
    let growth = after - settled;
    // The model's own conclusions must not grow on an unchanged round. Evidence and index rows may:
    // that is history, and it is kept. What must not accumulate is a new fact or action per round.
    let facts_after = count(&store, "SELECT count(*) FROM object_facts");
    let actions_after = count(&store, "SELECT count(*) FROM object_actions");
    // The plan asks for the database, WAL and index footprint separately, because a store that looks
    // small while its write-ahead log holds the rest is not a store with a small footprint.
    let sizes = components(&path)?;
    remove(&path);
    Ok(json!({
        "rounds": rounds,
        "items_per_round": 200,
        "bytes_settled": settled,
        "bytes_after": after,
        // Unchanged rounds may add evidence and index rows — that is history, not garbage. What
        // must not grow is the model's own conclusions.
        "bytes_growth": growth,
        "bytes_growth_per_round": growth / rounds.max(1) as i64,
        "counts_settled": counts,
        "facts_after": facts_after,
        "actions_after": actions_after,
        // The acceptance condition, stated as a value rather than left to a reader to compare.
        "facts_unchanged": facts_after == counts["facts"].as_i64().unwrap_or(-1),
        "actions_unchanged": actions_after == counts["actions"].as_i64().unwrap_or(-1),
        // SQLite keeps indexes inside the database file, so they are part of `database` rather than
        // a separate number; the write-ahead log and its shared-memory file are the ones that do
        // carry size outside it.
        "components_bytes": sizes,
        "elapsed_ms": elapsed,
        "elapsed_ms_per_round": elapsed / rounds.max(1) as u64,
    }))
}

/// A quota is honoured: the oldest history is removed, the store stays inside its limit, and what
/// remains is explainable.
fn rolling_cleanup(rounds: usize, quota_bytes: u64) -> Result<Value> {
    let path = scratch("rolling");
    let mut store = Store::open(&path)?;
    store.set_retention("rolling", quota_bytes)?;
    let mut ingested = 0;
    let mut refused = 0;
    for round in 0..rounds {
        // Each round is a genuinely different state, so nothing can be collapsed as a replay.
        match store.ingest(&snapshot(&format!("run-{round}"), 200, &format!("round{round}"))) {
            Ok(_) => ingested += 1,
            // A store that cannot hold what is necessary says so rather than pretending.
            Err(_) => refused += 1,
        }
        let _ = store.project_pending("M", "I", 10_000);
    }
    let live = bytes(&store);
    let dangling = count(&store, "SELECT count(*) FROM evidence_refs r WHERE r.pruned_at IS NULL AND NOT EXISTS (SELECT 1 FROM evidence e WHERE e.evidence_id=r.evidence_id)");
    let unexplained = count(&store, "SELECT count(*) FROM evidence_refs WHERE pruned_at IS NULL AND evidence_id NOT IN (SELECT evidence_id FROM evidence)");
    let result = json!({
        "quota_bytes": quota_bytes,
        "rounds_attempted": rounds,
        "rounds_ingested": ingested,
        "rounds_refused": refused,
        "bytes_live": live,
        "within_quota": live as u64 <= quota_bytes,
        "pruned_references": count(&store, "SELECT count(*) FROM evidence_refs WHERE pruned_at IS NOT NULL"),
        // A reference into material that is gone, with no statement that it was pruned, is the
        // dangling pointer this behaviour exists to avoid.
        "dangling_references": dangling,
        "references_without_a_reason": unexplained,
        "earliest_retained": store.retention()?["earliest_retained_observation"].clone(),
    });
    remove(&path);
    Ok(result)
}

/// Unlimited means unlimited: no schedule, no age, no size removes a thing.
fn unlimited_retention(rounds: usize) -> Result<Value> {
    let path = scratch("unlimited");
    let mut store = Store::open(&path)?;
    store.set_retention("unlimited", 5_000_000_000)?;
    for round in 0..rounds {
        store.ingest(&snapshot(&format!("run-{round}"), 100, &format!("round{round}")))?;
        let _ = store.project_pending("M", "I", 10_000);
    }
    let snapshots = count(&store, "SELECT count(*) FROM snapshots");
    let result = json!({
        "rounds": rounds,
        "snapshots_retained": snapshots,
        // Every round is still there. Anything less would mean history was deleted on a schedule the
        // user did not choose.
        "all_rounds_retained": snapshots >= rounds as i64,
        "bytes_live": bytes(&store),
        "pruned_units": store.retention()?["pruned_units"].clone(),
    });
    remove(&path);
    Ok(result)
}

/// A store that cannot hold what is necessary refuses rather than reporting success.
fn insufficient_space() -> Result<Value> {
    let path = scratch("tight");
    let mut store = Store::open(&path)?;
    // Far below what a round of this size needs, and below the store's own minimum.
    store.set_retention("rolling", 40 * 1024 * 1024)?;
    let mut refused = 0;
    let mut message = String::new();
    for round in 0..40 {
        match store.ingest(&snapshot(&format!("run-{round}"), 4000, &format!("round{round}"))) {
            Ok(_) => {}
            Err(error) => {
                refused += 1;
                if message.is_empty() {
                    message = error.to_string();
                }
                break;
            }
        }
        let _ = store.project_pending("M", "I", 100_000);
    }
    let result = json!({
        "refused": refused > 0,
        // Three things the message has to do: not be the storage engine's own words, name the limit
        // that was reached, and say what can be done about it. The engine's "database or disk is
        // full" does none of them, and reads as a failing disk rather than a setting.
        "message": message,
        "message_is_the_engine_error": message.contains("database or disk is full"),
        "message_names_the_limit": message.contains("容量上限") || message.contains("quota"),
        "message_names_a_way_out": message.contains("提高配额") || message.contains("无限增长") || message.contains("unlimited"),
        "bytes_live": bytes(&store),
    });
    remove(&path);
    Ok(result)
}

fn main() -> Result<()> {
    let output = std::env::args().nth(1).unwrap_or_else(|| "model/capacity-report.json".into());
    let rounds: usize = std::env::args().nth(2).and_then(|value| value.parse().ok()).unwrap_or(200);

    println!("capacity: steady state over {rounds} unchanged rounds…");
    let steady = steady_state(rounds)?;
    println!("  bytes settled {} → after {} (growth {})", steady["bytes_settled"], steady["bytes_after"], steady["bytes_growth"]);
    println!("capacity: rolling cleanup…");
    let rolling = rolling_cleanup(rounds, 64 * 1024 * 1024)?;
    println!("  {} of {} rounds ingested · within quota {} · dangling {}", rolling["rounds_ingested"], rolling["rounds_attempted"], rolling["within_quota"], rolling["dangling_references"]);
    println!("capacity: unlimited retention…");
    let unlimited = unlimited_retention(rounds.min(50))?;
    println!("  {} snapshots retained · all present {}", unlimited["snapshots_retained"], unlimited["all_rounds_retained"]);
    println!("capacity: insufficient space…");
    let tight = insufficient_space()?;
    println!("  refused {} · message names the limit {} · names a way out {}", tight["refused"], tight["message_names_the_limit"], tight["message_names_a_way_out"]);

    let document = json!({
        "generated_by": "crates/storage/examples/model-capacity.rs",
        "note": "对象模型自身的开销与容量行为。在临时目录的库上测得；未在真实盘、真实采集负载或长跑下测过。",
        "steady_state": steady,
        "rolling_cleanup": rolling,
        "unlimited_retention": unlimited,
        "insufficient_space": tight,
    });
    std::fs::write(&output, serde_json::to_string_pretty(&document)? + "\n")?;
    println!("capacity: written to {output}");
    Ok(())
}
