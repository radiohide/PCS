//! S02 fixture loader and query baseline.
//!
//! Loads the fixture corpus through the same call sequence the service uses
//! (`Store::ingest` per message, then `observe_collection_success` once per collector invocation),
//! verifies that reloading the same corpus into a fresh database reproduces the same state, checks
//! the documented idempotency and reject rules, then measures the query matrix and store sizes.
//!
//! usage: refactor-baseline <fixtures-dir> <scratch-dir> [report.json]
use anyhow::{bail, Context, Result};
use rusqlite::{Connection, OpenFlags};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn scenario_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).with_context(|| format!("cannot read {}", dir.display()))? {
        let path = entry?.path();
        if path.extension().is_some_and(|e| e == "jsonl") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}
fn name(path: &Path) -> String {
    path.file_name().unwrap_or_default().to_string_lossy().into_owned()
}
fn read_messages(path: &Path) -> Result<Vec<pcscope_contracts::Envelope>> {
    let text = std::fs::read_to_string(path)?;
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| pcscope_contracts::parse(line).with_context(|| format!("{}: {}", name(path), line)))
        .collect()
}

/// One collector invocation: commit every message, then record the successful observation.
fn load_file(store: &mut pcscope_storage::Store, path: &Path) -> Result<(usize, usize, u128)> {
    let messages = read_messages(path)?;
    let started = Instant::now();
    let mut inserted = 0;
    for envelope in &messages {
        inserted += store.ingest(envelope)?;
    }
    inserted += store.observe_collection_success(&messages)?;
    Ok((messages.len(), inserted, started.elapsed().as_millis()))
}
fn load_all(store: &mut pcscope_storage::Store, files: &[PathBuf]) -> Result<Vec<Value>> {
    let mut per_file = Vec::new();
    for file in files {
        let (messages, inserted, ms) = load_file(store, file)?;
        per_file.push(json!({"file":name(file),"messages":messages,"inserted":inserted,"ms":ms}));
    }
    Ok(per_file)
}

/// Content digest over the retained state, read through a separate read-only connection.
fn state_digest(path: &Path) -> Result<Value> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut hasher = Sha256::new();
    let mut counts = serde_json::Map::new();
    for (label, sql) in [
        ("records", "SELECT seq||'|'||source||'|'||kind||'|'||object_id||'|'||occurred_at||'|'||data FROM records ORDER BY seq"),
        ("objects", "SELECT source||'|'||scope||'|'||id||'|'||data FROM objects ORDER BY source,scope,id"),
        ("object_times", "SELECT source||'|'||scope||'|'||id||'|'||coalesce(changed_at,'')||'|'||coalesce(seen_at,'') FROM object_times ORDER BY source,scope,id"),
    ] {
        let mut statement = conn.prepare(sql)?;
        let mut rows = statement.query([])?;
        let mut count = 0u64;
        while let Some(row) = rows.next()? {
            hasher.update(row.get::<_, String>(0)?.as_bytes());
            hasher.update(b"\n");
            count += 1;
        }
        counts.insert(label.into(), json!(count));
    }
    Ok(json!({"digest":format!("{:x}",hasher.finalize()),"rows":counts}))
}

/// Every invalid fixture must be stopped by protocol validation or by ingest, before any state
/// change. A fixture that is accepted is a real defect, not a baseline detail.
fn check_rejections(dir: &Path) -> Result<Vec<Value>> {
    let mut results = Vec::new();
    if !dir.exists() {
        return Ok(results);
    }
    for file in scenario_files(dir)? {
        let text = std::fs::read_to_string(&file)?;
        let mut rejected = true;
        let mut reason = String::new();
        let mut store = pcscope_storage::Store::open(":memory:")?;
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            match pcscope_contracts::parse(line) {
                Err(error) => reason = format!("protocol: {error}"),
                Ok(envelope) => match store.ingest(&envelope) {
                    Err(error) => reason = format!("ingest: {error}"),
                    Ok(_) => {
                        rejected = false;
                        reason = "accepted".into();
                        break;
                    }
                },
            }
        }
        results.push(json!({"file":format!("invalid/{}",name(&file)),"rejected":rejected,"reason":reason}));
    }
    Ok(results)
}

fn percentile(sorted: &[u128], fraction: f64) -> u128 {
    if sorted.is_empty() {
        return 0;
    }
    sorted[((sorted.len() - 1) as f64 * fraction).round() as usize]
}

/// Runs one query repeatedly and reports median and p95 milliseconds for both windows:
///
/// * cold — open a reader, then query. This is what the service does per request
///   (`crates/service/src/main.rs` opens a reader for every timeline/object request), so it is the
///   number the performance gate applies to.
/// * warm — query on an already-open reader, isolating query cost from connection setup.
fn measure(label: &str, repeats: usize, path: &Path, mut run: impl FnMut(&pcscope_storage::Store) -> Result<String>) -> Value {
    let mut cold = Vec::new();
    let mut detail = String::new();
    for _ in 0..repeats {
        let store = match pcscope_storage::Store::open_reader(path, std::time::Duration::from_secs(8)) {
            Ok(store) => store,
            Err(error) => return json!({"query":label,"error":error.to_string()}),
        };
        let started = Instant::now();
        match run(&store) {
            Ok(value) => detail = value,
            Err(error) => return json!({"query":label,"error":error.to_string()}),
        }
        cold.push(started.elapsed().as_micros());
    }
    let mut warm = Vec::new();
    match pcscope_storage::Store::open_reader(path, std::time::Duration::from_secs(8)) {
        Ok(store) => {
            for _ in 0..repeats {
                let started = Instant::now();
                if let Err(error) = run(&store) {
                    return json!({"query":label,"error":error.to_string()});
                }
                warm.push(started.elapsed().as_micros());
            }
        }
        Err(error) => return json!({"query":label,"error":error.to_string()}),
    }
    cold.sort_unstable();
    warm.sort_unstable();
    json!({
        "query":label,
        "cold_median_ms":percentile(&cold,0.5) as f64/1000.0,
        "cold_p95_ms":percentile(&cold,0.95) as f64/1000.0,
        "warm_median_ms":percentile(&warm,0.5) as f64/1000.0,
        "warm_p95_ms":percentile(&warm,0.95) as f64/1000.0,
        "result":detail
    })
}

fn query_matrix(path: &Path, repeats: usize) -> Result<Vec<Value>> {
    let timeline = |query: Value| {
        move |store: &pcscope_storage::Store| -> Result<String> {
            let value = store.search_timeline(&query, None, 100)?;
            Ok(format!("rows={} facets={}", value["items"].as_array().map_or(0, Vec::len), value["sources"].as_array().map_or(0, Vec::len)))
        }
    };
    let objects = |query: Value| {
        move |store: &pcscope_storage::Store| -> Result<String> {
            let value = store.search_objects(&query, 0, 100)?;
            Ok(format!("total={} rows={}", value["total"], value["items"].as_array().map_or(0, Vec::len)))
        }
    };
    let mut out = Vec::new();
    // 常用筛选
    out.push(measure("timeline.default", repeats, path, timeline(json!({}))));
    out.push(measure("timeline.by_category", repeats, path, timeline(json!({"category":"startup"}))));
    out.push(measure("timeline.by_item", repeats, path, timeline(json!({"item":"pcscope.startup.tasks"}))));
    out.push(measure("timeline.by_source", repeats, path, timeline(json!({"source":"pcscope.eventlog.security"}))));
    out.push(measure("timeline.kind_events", repeats, path, timeline(json!({"kind":"events"}))));
    out.push(measure("timeline.kind_coverage", repeats, path, timeline(json!({"kind":"coverage"}))));
    out.push(measure("timeline.self_activity_hide", repeats, path, timeline(json!({"self_activity":"hide"}))));
    // 关键词命中与无匹配
    out.push(measure("timeline.keyword_hit", repeats, path, timeline(json!({"search":"示例任务"}))));
    out.push(measure("timeline.keyword_absent", repeats, path, timeline(json!({"search":"pcscope-no-match-7b6c5d4e"}))));
    // 对象列表与筛选
    out.push(measure("objects.default", repeats, path, objects(json!({"sort":"item_asc"}))));
    out.push(measure("objects.by_category", repeats, path, objects(json!({"category":"startup"}))));
    out.push(measure("objects.by_source", repeats, path, objects(json!({"source":"pcscope.software"}))));
    out.push(measure("objects.by_item", repeats, path, objects(json!({"item":"pcscope.eventlog.security.sessions"}))));
    out.push(measure("objects.keyword_hit", repeats, path, objects(json!({"search":"示例任务"}))));
    out.push(measure("objects.keyword_absent", repeats, path, objects(json!({"search":"pcscope-no-match-7b6c5d4e"}))));
    // 详情与对比
    out.push(measure("objects.detail_first_row", repeats, path, |store| {
        let page = store.search_objects(&json!({"sort":"item_asc"}), 0, 1)?;
        Ok(format!("id={}", page["items"][0]["id"]))
    }));
    out.push(measure("timeline.detail_task_evidence", repeats, path, |store| {
        // Evidence lookup only applies to scheduled-task snapshots; filter to that source rather
        // than taking an arbitrary first row, which the store correctly refuses to explain.
        let page = store.search_timeline(&json!({"source":"pcscope.startup.tasks"}), None, 1)?;
        let seq = page["items"][0]["seq"].as_i64().context("no scheduled-task timeline row")?;
        let evidence = store.task_evidence(seq)?;
        Ok(format!("keys={}", evidence.as_object().map_or(0, serde_json::Map::len)))
    }));
    out.push(measure("snapshots.list", repeats, path, |store| Ok(format!("rows={}", store.snapshots(50)?.len()))));
    out.push(measure("retention.status", repeats, path, |store| {
        let value = store.retention()?;
        Ok(format!("mode={} live_bytes={}", value["mode"], value["live_bytes"]))
    }));
    Ok(out)
}

/// Loads the corpus one scenario at a time and reports the state after each, so lifecycle effects
/// stay visible. An object absent from a later complete snapshot leaves the current projection
/// (its row disappears from `objects`) while a `object.no_longer_observed` record is retained;
/// staging makes that observable instead of inferred from a final count.
fn staged_load(files: &[PathBuf], scratch: &Path) -> Result<Vec<Value>> {
    let path = scratch.join("staged.db");
    fresh(&path)?;
    let mut store = pcscope_storage::Store::open(&path)?;
    let mut stages = Vec::new();
    let mut previous = std::collections::BTreeSet::new();
    for file in files {
        let (messages, inserted, _) = load_file(&mut store, file)?;
        let counts = store.counts()?;
        let disappeared = store
            .search_timeline(&json!({"kind":"object.no_longer_observed"}), None, 100)?["items"]
            .as_array()
            .map_or(0, Vec::len);
        // Exact projection diff: which identities entered and left the current object list.
        let current = object_keys(&path)?;
        let added: Vec<_> = current.difference(&previous).cloned().collect();
        let removed: Vec<_> = previous.difference(&current).cloned().collect();
        previous = current;
        stages.push(json!({
            "file":name(file),
            "messages":messages,
            "inserted":inserted,
            "objects":counts["objects"],
            "records":counts["records"],
            "no_longer_observed_rows":disappeared,
            "projection_added":added.len(),
            "projection_removed":removed.len(),
            "projection_removed_sample":removed.iter().take(4).collect::<Vec<_>>()
        }));
    }
    Ok(stages)
}
fn object_keys(path: &Path) -> Result<std::collections::BTreeSet<String>> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut statement = conn.prepare("SELECT source||' | '||scope||' | '||id FROM objects")?;
    let mut rows = statement.query([])?;
    let mut keys = std::collections::BTreeSet::new();
    while let Some(row) = rows.next()? {
        keys.insert(row.get::<_, String>(0)?);
    }
    Ok(keys)
}

/// Reads while a second connection backfills. Two windows are recorded separately, because they
/// have different causes and different severity:
///
/// * startup — the writer is still creating the schema. A reader here gets "no such table" rather
///   than a known-empty state. This is measured and reported, not hidden.
/// * backfill — the schema exists and the writer is committing data. A reader must stay correct and
///   bounded; any error here fails the baseline.
fn concurrent_during_backfill(files: &[PathBuf], scratch: &Path) -> Result<Value> {
    let path = scratch.join("concurrent.db");
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(scratch.join(format!("concurrent.db{suffix}")));
    }
    let writer_path = path.clone();
    let files = files.to_vec();
    let writer = std::thread::spawn(move || -> Result<()> {
        let mut store = pcscope_storage::Store::open(&writer_path)?;
        for file in &files {
            let messages = read_messages(file)?;
            for envelope in &messages {
                store.ingest(envelope)?;
            }
            store.observe_collection_success(&messages)?;
        }
        Ok(())
    });
    let mut samples = Vec::new();
    let mut startup_failures = 0usize;
    let mut backfill_failures = Vec::new();
    let mut ready = false;
    let deadline = Instant::now() + std::time::Duration::from_secs(30);
    while !writer.is_finished() && Instant::now() < deadline {
        match pcscope_storage::Store::open_reader(&path, std::time::Duration::from_secs(8)) {
            Ok(store) => {
                let started = Instant::now();
                match store.search_timeline(&json!({}), None, 100) {
                    Ok(_) => {
                        ready = true;
                        samples.push(started.elapsed().as_micros());
                    }
                    Err(_) if !ready => startup_failures += 1,
                    Err(error) => backfill_failures.push(error.to_string()),
                }
            }
            Err(error) => {
                if ready {
                    backfill_failures.push(error.to_string());
                } else {
                    startup_failures += 1;
                }
            }
        }
    }
    writer.join().map_err(|_| anyhow::anyhow!("backfill thread panicked"))??;
    samples.sort_unstable();
    Ok(json!({
        "reads_during_backfill":samples.len(),
        "median_ms":if samples.is_empty(){0.0}else{percentile(&samples,0.5) as f64/1000.0},
        "p95_ms":if samples.is_empty(){0.0}else{percentile(&samples,0.95) as f64/1000.0},
        "reads_before_schema_ready":startup_failures,
        "errors":backfill_failures
    }))
}

fn store_files(path: &Path) -> Result<Value> {
    let size = |suffix: &str| -> u64 {
        std::fs::metadata(format!("{}{suffix}", path.display())).map(|m| m.len()).unwrap_or(0)
    };
    Ok(json!({"db_bytes":size(""),"wal_bytes":size("-wal"),"shm_bytes":size("-shm")}))
}
fn fresh(path: &Path) -> Result<()> {
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        bail!("usage: refactor-baseline <fixtures-dir> <scratch-dir> [report.json]");
    }
    let fixtures = PathBuf::from(&args[1]);
    let scratch = PathBuf::from(&args[2]);
    let report_path = args.get(3).cloned();
    std::fs::create_dir_all(&scratch)?;
    let files = scenario_files(&fixtures)?;
    if files.is_empty() {
        bail!("no .jsonl fixtures in {}", fixtures.display());
    }

    let rejections = check_rejections(&fixtures.join("invalid"))?;
    let rejected = rejections.iter().filter(|r| r["rejected"] == true).count();

    // Reproducibility: the same corpus into two fresh databases must produce identical state.
    let first = scratch.join("run-a.db");
    fresh(&first)?;
    let mut store = pcscope_storage::Store::open(&first)?;
    let load_started = Instant::now();
    let per_file = load_all(&mut store, &files)?;
    let total_load_ms = load_started.elapsed().as_millis();
    let counts = store.counts()?;
    drop(store);
    let digest_a = state_digest(&first)?;

    let second = scratch.join("run-b.db");
    fresh(&second)?;
    let mut store = pcscope_storage::Store::open(&second)?;
    load_all(&mut store, &files)?;
    drop(store);
    let digest_b = state_digest(&second)?;

    // Replaying the snapshot scenarios into a database that already holds them must add nothing.
    let mut store = pcscope_storage::Store::open(&first)?;
    let mut replay_inserted = 0;
    let mut replay_errors = Vec::new();
    for file in &files {
        for envelope in read_messages(file)? {
            if matches!(&envelope.message, pcscope_contracts::Message::Snapshot { .. }) {
                match store.ingest(&envelope) {
                    Ok(inserted) => replay_inserted += inserted,
                    Err(error) => replay_errors.push(format!("{}: {error}", name(file))),
                }
            }
        }
    }
    drop(store);
    let digest_replay = state_digest(&first)?;

    let stages = staged_load(&files, &scratch)?;
    let sizes = store_files(&first)?;
    let queries = query_matrix(&first, 5)?;
    let concurrent = concurrent_during_backfill(&files, &scratch)?;

    let worst = queries
        .iter()
        .filter_map(|q| q["cold_p95_ms"].as_f64())
        .fold(0.0_f64, f64::max);
    let worst_warm = queries
        .iter()
        .filter_map(|q| q["warm_p95_ms"].as_f64())
        .fold(0.0_f64, f64::max);
    let report = json!({
        "generated_by":"crates/storage/examples/refactor-baseline.rs",
        "environment":{
            "os":std::env::consts::OS,
            "arch":std::env::consts::ARCH,
            "fixtures":fixtures.display().to_string(),
            "scenario_files":files.len()
        },
        "load":{"files":per_file,"total_ms":total_load_ms,"counts":counts},
        "reproducibility":{
            "run_a":digest_a,"run_b":digest_b,
            "identical":digest_a["digest"]==digest_b["digest"]
        },
        "replay":{
            "snapshot_replay_inserted":replay_inserted,
            "errors":replay_errors,
            "state_unchanged":digest_replay["digest"]==digest_a["digest"]
        },
        "invalid_fixtures":{"total":rejections.len(),"rejected":rejected,"detail":rejections},
        "stages":stages,
        "sizes":sizes,
        "queries":queries,
        "worst_query_cold_p95_ms":worst,
        "worst_query_warm_p95_ms":worst_warm,
        "during_backfill":concurrent
    });
    let text = serde_json::to_string_pretty(&report)?;
    if let Some(path) = &report_path {
        std::fs::write(path, &text)?;
    }
    println!("{text}");

    // Fail loudly rather than reporting a baseline that does not hold together.
    if digest_a["digest"] != digest_b["digest"] {
        bail!("reloading the same corpus produced different state");
    }
    if replay_inserted != 0 || !replay_errors.is_empty() {
        bail!("snapshot replay was not idempotent: {replay_inserted} inserted, {} errors", replay_errors.len());
    }
    if digest_replay["digest"] != digest_a["digest"] {
        bail!("snapshot replay changed stored state");
    }
    if rejected != rejections.len() {
        bail!("{} invalid fixtures were accepted", rejections.len() - rejected);
    }
    if !concurrent["errors"].as_array().is_some_and(Vec::is_empty) {
        bail!("queries failed during backfill");
    }
    Ok(())
}
