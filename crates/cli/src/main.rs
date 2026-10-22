use anyhow::{bail, Context, Result};
use std::io::{self, BufRead, Read, Write};
fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        bail!("usage: pcscope-cli <init-baseline|ingest|timeline|collect|install-read-indexes> <database-path> [collector-exe|seconds]");
    }
    if args[1]=="install-read-indexes" {
        println!("{}",pcscope_storage::Store::install_read_indexes(&args[2])?);
        return Ok(());
    }
    if args[1]=="init-baseline" {
        anyhow::ensure!(!std::path::Path::new(&args[2]).exists(), "请指定新数据库路径；不会重置已有数据库");
        let seconds: u64 = args.get(3).context("采集周期（秒）必填")?.parse()?;
        anyhow::ensure!((30..=604800).contains(&seconds), "采集周期应为 30–604800 秒");
        let mut store = pcscope_storage::Store::open_program(&args[2])?;
        println!("{}", store.initialize_collection_baseline(seconds)?);
        return Ok(());
    }
    let mut store = if matches!(args[1].as_str(),"timeline"|"reconcile") {
        pcscope_storage::Store::open_reader(&args[2],std::time::Duration::from_secs(6))?
    } else {pcscope_storage::Store::open_program(&args[2])?};
    match args[1].as_str() {
        "import-evtx" => {
            let archive = std::fs::canonicalize(args.get(3).context("EVTX file required")?)?;
            // Use ordinary Windows paths: Win32 event APIs do not uniformly accept extended paths.
            let path = archive
                .to_string_lossy()
                .trim_start_matches(r"\\?\")
                .to_owned();
            let executable = std::env::current_exe()?.with_file_name("pcscope-eventlog.exe");
            let mut resume = None;
            let mut initialized = false;
            loop {
                let input = serde_json::to_vec(
                        &serde_json::json!({"channel":"Application","archive_path":path,"resume":resume,"watch":store.file_watch_settings()?}),
                )?;
                let result = pcscope_supervisor::collect_with_input(
                    &executable,
                    &[],
                    Some(input),
                    std::time::Duration::from_secs(40),
                )?;
                let mut more = false;
                for envelope in result.messages {
                    match &envelope.message {
                        pcscope_contracts::Message::EventBatch {
                            source, checkpoint, ..
                        } => {
                            if !initialized {
                                initialized = true;
                                if let Some(saved) = store.source_resume(source)? {
                                    resume = Some(saved);
                                    more = true;
                                    break;
                                }
                            }
                            let inserted = store.ingest(&envelope)?;
                            eprintln!("EVTX committed {inserted} records");
                            resume = store.source_resume(source)?;
                            more = serde_json::from_str::<serde_json::Value>(checkpoint)?
                                ["has_more"]
                                .as_bool()
                                .unwrap_or(false);
                        }
                        pcscope_contracts::Message::Gap { reason, .. } => {
                            store.ingest(&envelope)?;
                            bail!("EVTX import incomplete: {reason}");
                        }
                        _ => bail!("unexpected importer message"),
                    }
                }
                if !more {
                    break;
                }
            }
        }
        "collect" => {
            let executable = args.get(3).context("collector executable required")?;
            eprintln!("Collecting: {}", executable);
            let result = pcscope_supervisor::collect(
                std::path::Path::new(executable),
                std::time::Duration::from_secs(60),
            )?;
            let count = result.messages.len();
            for (index, envelope) in result.messages.iter().enumerate() {
                let inserted = store.ingest(envelope)?;
                eprintln!("[{}/{}] committed {} changes", index + 1, count, inserted);
            }
            if !result.diagnostic.is_empty() {
                eprintln!(
                    "Collector emitted {} diagnostic bytes",
                    result.diagnostic.len()
                );
            }
        }
        "ingest" => {
            let mut input = io::stdin().lock();
            loop {
                let mut bytes = Vec::new();
                let n = std::io::Read::by_ref(&mut input)
                    .take((pcscope_contracts::MAX_LINE_BYTES + 1) as u64)
                    .read_until(b'\n', &mut bytes)?;
                if n == 0 {
                    break;
                }
                if bytes.len() > pcscope_contracts::MAX_LINE_BYTES {
                    bail!("protocol line exceeds limit");
                }
                let envelope = pcscope_contracts::parse(
                    std::str::from_utf8(&bytes).context("protocol must be UTF-8")?,
                )?;
                let inserted = store.ingest(&envelope)?;
                println!(
                    "{}",
                    serde_json::json!({"type":"ack","protocol_version":1,"request_id":envelope.request_id,"inserted":inserted})
                );
                io::stdout().flush()?;
            }
        }
        // S28. Reconciliation reports per input rather than per total, so the summary line says how
        // many inputs have no status and the verdict says whether to look further.
        "reconcile" => {
            let report = store.reconcile()?;
            let outstanding: i64 = report["inputs"].as_array().map(|rows| rows.iter().map(|row| row["unqueued"].as_i64().unwrap_or(0) + row["pending"].as_i64().unwrap_or(0)).sum()).unwrap_or(0);
            eprintln!(
                "reconcile: verdict {} · {} input(s) without a decision · {} failure(s) · {} dangling reference(s)",
                pcscope_storage::reconcile::verdict(&report),
                outstanding,
                report["watermarks"]["failed"],
                report["field_trace"]["dangling_references"]
            );
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        "timeline" => println!(
            "{}",
            serde_json::to_string_pretty(&store.timeline(None, 100)?)?
        ),
        // S27's `migrate` subcommand was removed on 2026-09-12: the user decided not to migrate the
        // old data (docs/full-object-refactor-plan.md §19). The legacy store is preserved by
        // scripts/archive-legacy-store.cjs instead, and stays in place unprojected.
        _ => bail!("unknown command"),
    }
    Ok(())
}
