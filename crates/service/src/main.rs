mod notification_scope;
use anyhow::{Context, Result};
use pcscope_ipc::Request;
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
mod schedule;
mod monitoring;
mod projection;
mod event_backoff;
mod runtime;
mod runtime_environment;
mod device_notify;
mod read_api;
mod replacement;
mod startup_report;
#[cfg(all(test, windows))]
mod read_api_tests;

struct State {
    endpoint: pcscope_ipc::Endpoint,
    query_slots: tokio::sync::Semaphore,
    monitoring: Mutex<Value>,
    checking: AtomicBool,
    monitoring_writes: AtomicUsize,
    data_dir: PathBuf,
    schedule: Mutex<schedule::Schedule>,
    store: Arc<Mutex<pcscope_storage::Store>>,
    projection_ready: Arc<AtomicBool>,
    projection_status: Arc<Mutex<Value>>,
    collecting: AtomicBool,
    last_result: Mutex<Value>,
    collectors: Vec<Collector>,
    event_collector: PathBuf,
    polling: AtomicBool,
    stopping: AtomicBool,
    event_result: Mutex<Value>,
    usn_result: Mutex<Value>,
    usn_exclusions: Vec<String>,
    shutdown: tokio::sync::Notify,
    channel_cursor: AtomicUsize,
    event_backoff: Mutex<event_backoff::Backoff>,
    check_after: Mutex<std::time::Instant>,
}
struct Collector {
    path: PathBuf,
    domain: Option<String>,
}
impl Collector {
    fn name(&self) -> String {
        self.domain.clone().unwrap_or_else(|| {
            self.path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        })
    }
}

fn collect_owned(state:&State,path:&std::path::Path,args:&[std::ffi::OsString],input:Option<Vec<u8>>,timeout:Duration,plugin:&str)->Result<pcscope_supervisor::Collection>{
    let mut selection=state.store.lock().unwrap().collection_selection(if plugin.starts_with("eventlog/"){"eventlog"}else if plugin=="runtime"{"scoped"}else{plugin})?;
    if let Some(sources)=state.schedule.lock().unwrap().active_sources(plugin){notification_scope::restrict(&mut selection,plugin,&sources)?;}
    if selection["all_disabled"]==true{return Ok(pcscope_supervisor::Collection{messages:vec![],diagnostic:String::new()});}
    let input=if plugin=="pcscope-software.exe"{input}else{
        let mut request:Value=input.as_ref().map(|v|serde_json::from_slice(v)).transpose()?.unwrap_or(json!({}));
        let mut selection=selection;
        if plugin=="scoped" {
            // Runtime owns these sources in a separate bounded lane; keep the existing controls.
            let disabled=selection["disabled_sources"].as_array_mut().context("source selection missing")?;
            for source in runtime::SOURCES {if !disabled.contains(&json!(source)){disabled.push(json!(source));}}
        }
        if let Some(channel)=request["channel"].as_str(){let source=format!("pcscope.eventlog.{}",channel.to_lowercase());if selection["disabled_sources"].as_array().is_some_and(|a|a.contains(&json!(source))){return Ok(pcscope_supervisor::Collection{messages:vec![],diagnostic:String::new()});}selection["event_rules"].as_array_mut().unwrap().retain(|r|r["source"]==source);}
        request["selection"]=selection;Some(serde_json::to_vec(&request)?)
    };
    let run_id=format!("{plugin}/{}",chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default());
    pcscope_supervisor::collect_observed(path,args,input,timeout,&mut |mut processes|{
        if let Ok(identity)=pcscope_supervisor::process_identity(std::process::id()){processes.push(identity);}
        let store=state.store.lock().map_err(|_|anyhow::anyhow!("storage lock poisoned"))?;
        for p in processes {let service=p.pid==std::process::id();store.observe_own_process(p.pid,p.created_ms,p.seen_ms,p.ended_ms,&p.image,if service{"service"}else if p.is_root{"collector"}else{"descendant"},if service{"pcscope-service"}else{plugin},if service{"service"}else{&run_id})?;}
        Ok(())
    })
}

// Process/timeout/protocol failures must survive a UI refresh or daemon restart.
fn persist_collector_failure(state: &State, source: &str, error: &anyhow::Error) {
    let observed_at = chrono::Utc::now().to_rfc3339();
    let envelope = pcscope_contracts::Envelope {
        protocol_version: 1,
        request_id: format!("collector-failure/{source}/{observed_at}"),
        message: pcscope_contracts::Message::Gap {
            source: source.into(), observed_at,
            reason: format!("Collector failed; this run is not complete. Previously committed scopes remain valid only at their observation times: {error}"),
        },
    };
    match state.store.lock() {
        Ok(mut store) => if let Err(error) = store.ingest(&envelope) { eprintln!("Unable to persist collector gap: {error}"); },
        Err(_) => eprintln!("Unable to persist collector gap: storage lock poisoned"),
    }
}

fn start_event_collection(state: Arc<State>, manual: Option<&str>) -> Result<()> {
    if !state.projection_ready.load(Ordering::SeqCst) {
        if manual.is_some() { anyhow::bail!("对象投影正在恢复，请等待积压处理完成"); }
        return Ok(());
    }
    if state.stopping.load(Ordering::SeqCst) {
        anyhow::bail!("后台正在停止");
    }
    let ids = state.schedule.lock().unwrap().reserve(true, manual)?;
    if ids.is_empty() {
        return Ok(());
    }
    state.polling.store(true, Ordering::SeqCst);
    let manual_run=manual.is_some();
    tokio::task::spawn_blocking(move || {
        if ids.iter().any(|id| id == "eventlog") && state.schedule.lock().unwrap().begin("eventlog")
        {
            let mut outcomes = Vec::new();
            let channels: Vec<String> =
                serde_json::from_str(include_str!("../../../plugins/eventlog/channels.json"))
                    .expect("bundled channel manifest");
            let cursor = state.channel_cursor.fetch_add(1, Ordering::SeqCst);
            let channel_indices=state.schedule.lock().unwrap().event_channel_indices(channels.len(),cursor);
            for channel in channel_indices.iter().map(|index|&channels[*index]) {
                if state.stopping.load(Ordering::SeqCst) {
                    break;
                }
                if let Some(cooling)=state.event_backoff.lock().unwrap().cooling(channel,manual_run) {outcomes.push(cooling);continue;}
                let result = (|| -> Result<Value> {
                    let mut count = 0;
                    let mut gaps = Vec::new();
                    let mut has_more = true;
                    for _ in 0..3 {
                    if state.stopping.load(Ordering::SeqCst) {break;}
                    let resume = state
                        .store
                        .lock()
                        .map_err(|_| anyhow::anyhow!("storage lock poisoned"))?
                        .source_resume(&format!("pcscope.eventlog.{}", channel.to_lowercase()))?;
                    let watch=state.store.lock().unwrap().file_watch_settings()?;
                    let item_revision=state.store.lock().unwrap().item_settings()?["revision"].clone();
                    let started_at=state.store.lock().unwrap().collection_started_at()?;
                    let input = serde_json::to_vec(&json!({"channel":channel,"resume":resume,"watch":watch,"started_at":started_at}))?;
                    let collected = collect_owned(
                        &state, &state.event_collector,
                        &[],
                        Some(input),
                        Duration::from_secs(40), &format!("eventlog/{channel}"),
                    )?;
                    let mut committed=true;
                    for message in &collected.messages {
                        if let pcscope_contracts::Message::Gap { reason, .. } = &message.message {
                            gaps.push(reason.clone());
                        }
                        let mut store=state.store.lock().map_err(|_|anyhow::anyhow!("storage lock poisoned"))?;
                        if store.file_watch_settings()?["revision"]!=watch["revision"] {committed=false;break;}
                        if store.item_settings()?["revision"]!=item_revision {committed=false;break;}
                        count += store.ingest(&message)?;
                    }
                    if committed {count += state.store.lock().unwrap().observe_collection_success(&collected.messages)?;}
                    let resume=state.store.lock().unwrap().source_resume(&format!("pcscope.eventlog.{}", channel.to_lowercase()))?;
                    has_more=resume.as_ref().and_then(|r|r["checkpoint"].as_str()).and_then(|s|serde_json::from_str::<Value>(s).ok()).is_none_or(|c|c["has_more"]!=false);
                    if !gaps.is_empty() || !has_more {break;}
                    }
                    // `gap`, not `blocked`: this channel reported gaps, which is the same fact the
                    // coverage table records as `gap`. Two words for one fact is how the frontend
                    // ends up with one of them falling through to a fallback label.
                    Ok(json!({"channel":channel,"ok":gaps.is_empty(),"changes":count,"gaps":gaps,"has_more":has_more,"history_state":if !gaps.is_empty(){"gap"}else if has_more{"catching_up"}else{"caught_up"}}))
                })();
                let mut outcome=result.unwrap_or_else(|e| {
                    persist_collector_failure(&state, &format!("pcscope.eventlog.{}", channel.to_lowercase()), &e);
                    json!({"channel":channel,"ok":false,"error":e.to_string()})
                });
                state.event_backoff.lock().unwrap().record(channel,&mut outcome);outcomes.push(outcome);
            }
            let summary = json!({"ok":outcomes.iter().all(|v|v["ok"]==true),"channels":outcomes});
            if let Ok(mut last) = state.event_result.lock() {
                if !last.is_object() {
                    *last = json!({"channels":{}});
                }
                for mut outcome in summary["channels"].as_array().unwrap().clone() {
                    let channel = outcome["channel"].as_str().unwrap_or("unknown").to_owned();
                    outcome["at"] = json!(chrono::Utc::now().to_rfc3339());
                    last["channels"][channel] = outcome;
                }
            }
            state.schedule.lock().unwrap().finish("eventlog", summary);
        }
        if !state.stopping.load(Ordering::SeqCst)
            && ids.iter().any(|id| id == "usn")
            && state.schedule.lock().unwrap().begin("usn")
        {
            let result = (|| -> Result<Value> {
                let mut resumes = serde_json::Map::new();
                {
                    let store = state
                        .store
                        .lock()
                        .map_err(|_| anyhow::anyhow!("storage lock poisoned"))?;
                    for drive in b'a'..=b'z' {
                        let source = format!("pcscope.usn.{}", drive as char);
                        if let Some(resume) = store.source_resume(&source)? {
                            resumes.insert(source, resume);
                        }
                    }
                }
                let watch=state.store.lock().unwrap().file_watch_settings()?;
                let item_revision=state.store.lock().unwrap().item_settings()?["revision"].clone();
                let result = collect_owned(
                    &state, &state.event_collector.with_file_name("pcscope-usn.exe"),
                    &[],
                    Some(serde_json::to_vec(
                        &json!({"resumes":resumes,"exclude_files":state.usn_exclusions,"watch":watch}),
                    )?),
                    Duration::from_secs(40), "usn",
                )?;
                let mut changes = 0;
                let mut gaps = Vec::new();
                let mut committed=true;
                for message in &result.messages {
                    if let pcscope_contracts::Message::Gap { source, reason, .. } = &message.message
                    {
                        let gap = json!({"source":source,"reason":reason});
                        gaps.push(gap);
                    }
                    let mut store=state.store.lock().map_err(|_|anyhow::anyhow!("storage lock poisoned"))?;
                    if store.file_watch_settings()?["revision"]!=watch["revision"] {committed=false;break;}
                    if store.item_settings()?["revision"]!=item_revision {committed=false;break;}
                    changes += store.ingest(&message)?;
                }
                if committed {changes += state.store.lock().unwrap().observe_collection_success(&result.messages)?;}
                Ok(
                    json!({"ok":gaps.is_empty(),"changes":changes,"gaps":gaps,"at":chrono::Utc::now().to_rfc3339()}),
                )
            })();
            // Persisting a failure can wait for the writer. Never hold the status value's lock
            // during that wait: Status must remain readable while storage is busy.
            let result=result.unwrap_or_else(|e|{persist_collector_failure(&state, "pcscope.usn.collector", &e);json!({"ok":false,"error":e.to_string(),"at":chrono::Utc::now().to_rfc3339()})});
            if let Ok(mut last) = state.usn_result.lock() { *last=result.clone(); }
            state.schedule.lock().unwrap().finish("usn", result);
        }
        state.schedule.lock().unwrap().cancel(&ids);
        state.polling.store(false, Ordering::SeqCst);
    });
    Ok(())
}

fn start_collection(state: Arc<State>, manual: Option<&str>) -> Result<()> {
    if !state.projection_ready.load(Ordering::SeqCst) {
        if manual.is_some() { anyhow::bail!("对象投影正在恢复，请等待积压处理完成"); }
        return Ok(());
    }
    if state.stopping.load(Ordering::SeqCst) {
        anyhow::bail!("后台正在停止");
    }
    let ids = state.schedule.lock().unwrap().reserve(false, manual)?;
    if ids.is_empty() {
        return Ok(());
    }
    state.collecting.store(true, Ordering::SeqCst);
    tokio::task::spawn_blocking(move || {
        let mut outcomes = Vec::new();
        let mut total = 0;
        for (index, collector) in state
            .collectors
            .iter()
            .filter(|c| ids.contains(&c.name()))
            .enumerate()
        {
            if state.stopping.load(Ordering::SeqCst) {
                break;
            }
            if !state.schedule.lock().unwrap().begin(&collector.name()) {
                continue;
            }
            if let Ok(mut last) = state.last_result.lock() {
                *last = json!({"operation":collector.name(),"completed":index,"total":ids.len()});
            }
            let result = (|| -> Result<(usize, Vec<String>)> {
                let targeted=state.schedule.lock().unwrap().active_sources(&collector.name()).is_some();
                let args: Vec<std::ffi::OsString> = if targeted {vec![collector.name().into()]}else{
                    collector.domain.iter().map(Into::into).collect()};
                let path=if targeted{collector.path.with_file_name("pcscope-platform-native.exe")}else{collector.path.clone()};
                let watch=if collector.domain.as_deref()==Some("scoped") {
                    let store=state.store.lock().unwrap();
                    let mut watch=store.file_watch_settings()?;let directories=store.directory_watch_settings()?;
                    watch["directories"]=directories["directories"].clone();watch["directory_revision"]=directories["revision"].clone();
                    watch["internal_data_dir"]=json!(state.data_dir);
                    Some(watch)
                }else{None};
                let item_revision=state.store.lock().unwrap().item_settings()?["revision"].clone();
                let input=watch.as_ref().map(serde_json::to_vec).transpose()?;
                let result = collect_owned(
                    &state, &path,
                    &args,
                    input,
                    Duration::from_secs(120), &collector.name(),
                )?;
                let mut count = 0;
                let mut gaps = Vec::new();
                let mut committed=true;
                for message in &result.messages {
                    if let pcscope_contracts::Message::Gap { reason, .. } = &message.message {
                        gaps.push(reason.clone());
                    }
                    let mut store=state.store.lock().map_err(|_|anyhow::anyhow!("storage lock poisoned"))?;
                    if let Some(watch)=&watch {if store.file_watch_settings()?["revision"]!=watch["revision"] || store.directory_watch_settings()?["revision"]!=watch["directory_revision"] {committed=false;break;}}
                    if store.item_settings()?["revision"]!=item_revision {committed=false;break;}
                    count += store.ingest(&message)?;
                }
                if committed {count += state.store.lock().unwrap().observe_collection_success(&result.messages)?;}
                Ok((count, gaps))
            })();
            outcomes.push(match result {
                Ok((n, gaps)) => {
                    total += n;
                    json!({"plugin":collector.name(),"ok":gaps.is_empty(),"changes":n,"gaps":gaps})
                }
                Err(e) => {
                    persist_collector_failure(&state, &format!("pcscope.collector.{}", collector.name()), &e);
                    json!({"plugin":collector.name(),"ok":false,"error":e.to_string()})
                },
            });
            state
                .schedule
                .lock()
                .unwrap()
                .finish(&collector.name(), outcomes.last().unwrap().clone());
        }
        if let Ok(mut last) = state.last_result.lock() {
            *last = json!({"ok":outcomes.iter().all(|v|v["ok"]==true),"changes":total,"plugins":outcomes});
        }
        state.schedule.lock().unwrap().cancel(&ids);
        state.collecting.store(false, Ordering::SeqCst);
    });
    Ok(())
}

fn dispatch(state: &Arc<State>, request: Request) -> Result<Value> {
    if read_api::handles(&request) { return read_api::dispatch(state, &request); }
    // A shutdown acknowledgment contains no source data and must not wait for a writer or even
    // require a readable database after notifying the graceful shutdown path.
    if matches!(&request, Request::Shutdown) { return dispatch_raw(state, request); }
    // The object model's query entry point is the only query-shaped request left. It reads through
    // its own independent reader, so a long query never takes the collection writer's lock. The
    // eight legacy read variants (timeline, timeline_search, objects, object_search, snapshots,
    // compare, task_evidence, event_related) were removed on 2026-09-12 — see
    // docs/full-object-refactor-plan.md §19.
    if matches!(&request,Request::Query{..}) {
        let _slot=state.query_slots.try_acquire().map_err(|_|anyhow::anyhow!("查询繁忙，请稍后重试；后台采集继续运行"))?;
        let store=pcscope_storage::Store::open_reader(state.data_dir.join("history.db"),Duration::from_secs(6))?;
        let Request::Query{request}=request else {unreachable!()};
        let result=store.query(&request).and_then(|answer|Ok(serde_json::to_value(answer)?));
        let mut value=result.map_err(|e|if e.to_string().contains("interrupted"){anyhow::anyhow!("查询超时，已停止本次查询。请增加采集项或变化类型筛选后重试；历史数据保留。")}else{e})?;
        pcscope_contracts::sensitive::present(&mut value,store.show_sensitive()?);
        return Ok(value);
    }
    let mut value = dispatch_raw(state, request)?;
    let reveal = read_api::reader(state)?.show_sensitive()?;
    pcscope_contracts::sensitive::present(&mut value, reveal);
    Ok(value)
}

fn check_monitoring(state: Arc<State>) -> bool {
    if state.stopping.load(Ordering::SeqCst) { return false; }
    let mut after=state.check_after.lock().unwrap();
    if *after>std::time::Instant::now() {return false;}
    if state.checking.swap(true, Ordering::SeqCst) {return false;}
    // Register the work before the final stop check, so shutdown cannot pass its wait between
    // observing stopping=false and marking this check active.
    if state.stopping.load(Ordering::SeqCst) {state.checking.store(false,Ordering::SeqCst);return false;}
    *after=std::time::Instant::now()+Duration::from_secs(30);drop(after);
    tokio::task::spawn_blocking(move || {
        let result=read_api::reader(&state).and_then(|store|store.monitoring_settings())
            .and_then(|settings|monitoring::check(settings["mode"]=="enhanced" && settings["repair"]==true))
            .unwrap_or_else(|e|json!({"error":e.to_string(),"checked_at":chrono::Utc::now().to_rfc3339()}));
        finish_monitoring(&state,result);
    });
    true
}

fn finish_monitoring(state: &State, result: Value) {
    // Publish promptly, but graceful shutdown must still wait for the history write. The counter
    // also covers a later check completing while an earlier result waits for the storage writer.
    state.monitoring_writes.fetch_add(1,Ordering::SeqCst);
    struct PendingWrite<'a>(&'a AtomicUsize);
    impl Drop for PendingWrite<'_> {fn drop(&mut self){self.0.fetch_sub(1,Ordering::SeqCst);}}
    let _pending=PendingWrite(&state.monitoring_writes);
    *state.monitoring.lock().unwrap()=result.clone();
    state.checking.store(false,Ordering::SeqCst);
    if let Ok(mut store)=state.store.lock() {
        let _=store.internal_event("collector.capability_check",&chrono::Utc::now().to_rfc3339(),result);
    }
}

fn dispatch_raw(state: &Arc<State>, request: Request) -> Result<Value> {
    match request {
        Request::SetItemEnabled{plugin,id,enabled}=>state.store.lock().unwrap().set_item_enabled(&plugin,&id,enabled,&chrono::Utc::now().to_rfc3339()),
        Request::RegisterFrontend {pid} => {
            let p=pcscope_supervisor::process_identity(pid)?;
            let expected=std::env::current_exe()?.with_file_name("pcscope-desktop.exe").canonicalize()?;
            if std::path::Path::new(&p.image).canonicalize()? != expected {anyhow::bail!("frontend executable does not match this installation");}
            state.store.lock().unwrap().observe_own_process(p.pid,p.created_ms,p.seen_ms,p.ended_ms,&p.image,"frontend","pcscope-desktop",&format!("frontend/{}/{}",p.pid,p.created_ms))?;
            Ok(json!({"registered":true}))
        },
        Request::SetDirectoryWatchSettings {directories} => state.store.lock().unwrap().set_directory_watch_settings(directories,&chrono::Utc::now().to_rfc3339()),
        Request::SetRuntimeWatchSettings {enabled,module_process_names} => state.store.lock().unwrap().set_runtime_watch_settings(enabled,module_process_names,&chrono::Utc::now().to_rfc3339()),
        Request::SetFileWatchSettings {files} => {
            state.store.lock().unwrap().set_file_watch_settings(files,&chrono::Utc::now().to_rfc3339())
        },
        Request::CheckMonitoring => {let started=check_monitoring(state.clone()); Ok(json!({"started":started,"coalesced":!started}))},
        Request::SetMonitoringSettings {mode,repair} => {
            let value=monitoring::settings(&mode,repair)?;
            if state.checking.load(Ordering::SeqCst) {anyhow::bail!("能力检查正在进行，请完成后再更改运行模式");}
            state.store.lock().unwrap().set_monitoring_settings(&value,&chrono::Utc::now().to_rfc3339())?;
            Ok(value)
        },
        Request::SetPrivacySettings { show_sensitive } => {
            state.store.lock().unwrap().set_show_sensitive(show_sensitive)?;
            Ok(json!({"show_sensitive":show_sensitive}))
        },
        Request::Plugins => Ok(state.schedule.lock().unwrap().view()),
        Request::GlobalCollectionSchedule => Ok(state.schedule.lock().unwrap().global_view()),
        Request::SetGlobalCollectionSchedule { seconds } => {
            let store = state.store.lock().map_err(|_| anyhow::anyhow!("storage lock poisoned"))?;
            let mut schedule = state.schedule.lock().unwrap();
            let config = schedule.global_candidate(seconds)?;
            store.save_plugin_settings(&config)?;
            schedule.configure_global(seconds);
            Ok(json!({"settings":schedule.global_view(),"jobs":schedule.view()}))
        }
        Request::SetPlugin {
            id,
            enabled,
            seconds,
        } => {
            // Wait for storage before taking the short-lived schedule lock. Plugins/status reads
            // must not inherit a collector's long writer wait.
            let store=state.store.lock().map_err(|_| anyhow::anyhow!("storage lock poisoned"))?;
            let mut schedule = state.schedule.lock().unwrap();
            let job = schedule.jobs.iter().find(|job|job.id==id).ok_or_else(||anyhow::anyhow!("未知插件"))?;
            let enabled = enabled.unwrap_or(job.enabled);
            if let Some(seconds) = seconds {
                let config = schedule.candidate(&id, enabled, seconds)?;
                store.save_plugin_settings(&config)?;
                schedule.configure(&id, enabled, seconds);
            } else {
                let mut config = schedule.config();
                config[&id]["enabled"] = json!(enabled);
                store.save_plugin_settings(&config)?;
                schedule.set_enabled(&id, enabled);
            }
            Ok(json!({"saved":true,"jobs":schedule.view()}))
        }
        Request::CollectPlugin { id } => {
            if !state
                .schedule
                .lock()
                .unwrap()
                .jobs
                .iter()
                .any(|j| j.id == id)
            {
                anyhow::bail!("未知插件");
            }
            if id == "eventlog" || id == "usn" {
                start_event_collection(state.clone(), Some(&id))?;
            } else {
                start_collection(state.clone(), Some(&id))?;
            }
            Ok(json!({"scheduled":true,"plugin":id}))
        }
        Request::Shutdown => {
            state.stopping.store(true, Ordering::SeqCst);
            state.shutdown.notify_one();
            Ok(json!({"stopping":true}))
        }
        Request::Status | Request::MonitoringSettings | Request::PrivacySettings |
        Request::ItemControls | Request::DirectoryWatchSettings | Request::FileWatchSettings |
        Request::RuntimeWatchSettings => anyhow::bail!("settings and status requests are answered by the read-only path"),
        // Answered by the read-only path above, which opens its own reader and never takes the
        // collection writer's lock. Reaching here would mean that path stopped matching it.
        Request::Query{..} => anyhow::bail!("query requests are answered by the read-only path"),
        Request::Collect => {
            start_collection(state.clone(), Some("*"))?;
            Ok(json!({"scheduled":true}))
        }
        Request::Retention => anyhow::bail!("retention is answered by the read-only path"),
        Request::RepairSnapshotAppearance { limit } => {
            anyhow::ensure!(!state.stopping.load(Ordering::SeqCst),"后台正在退出，请稍后重试");
            state.store.try_lock().map_err(|_|anyhow::anyhow!("后台正在处理采集，请稍后重试"))?.repair_snapshot_appearance(limit)
        }
        Request::SetRetention { mode, quota_bytes } => state
            .store
            .lock()
            .map_err(|_| anyhow::anyhow!("storage lock poisoned"))?
            .set_retention(&mode, quota_bytes),
    }
}

#[cfg(windows)]
fn main() -> Result<()> {
    let data = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .context("usage: pcscope-service <data-directory>; development user daemon only")?;
    std::fs::create_dir_all(&data)?;
    let endpoint = pcscope_ipc::Endpoint::for_directory(&data)?;
    let data = endpoint.data_dir.clone();
    let mut report=startup_report::Report::new(&data);
    let result=(||->Result<()> {
    report.phase("preparing_temporary_directory");
    let temp=runtime_environment::prepare_temp_directory(&data)?;
    // Set process-local paths before creating worker threads. SQLite's Windows VFS and
    // child collectors must not depend on an inaccessible inherited system temp directory.
    std::env::set_var("TEMP",&temp);
    std::env::set_var("TMP",&temp);
    tokio::runtime::Builder::new_multi_thread().enable_all().build()?.block_on(run(data,endpoint,&mut report))
    })();
    report.finish(&result);result
}

#[cfg(windows)]
async fn run(data:PathBuf,endpoint:pcscope_ipc::Endpoint,report:&mut startup_report::Report)->Result<()> {
    report.phase("reserving_connection");
    let await_previous = if std::env::args().nth(2).as_deref()==Some("--replace") {
        let expected=std::env::args().nth(3).context("replacement requires original user SID")?;
        anyhow::ensure!(pcscope_ipc::user_sid()?==expected,"请使用同一 Windows 账户提权，不能切换账户访问加密历史");
        replacement::request_shutdown(&endpoint).await?
    } else { false };
    // Reserve the pipe before opening the database or starting collectors.
    let deadline=std::time::Instant::now()+Duration::from_secs(180);
    let mut server = loop {
        match pcscope_ipc::server(&endpoint, true) {
            Ok(server)=>break server,
            Err(error) if await_previous && replacement::pipe_still_owned(&error) && std::time::Instant::now()<deadline=>{tokio::time::sleep(Duration::from_millis(200)).await;},
            Err(error)=>return Err(error).context("cannot reserve user pipe; another daemon may already be running"),
        }
    };
    let executable = std::env::current_exe()?;
    let mut collectors: Vec<Collector> = vec![
        executable.with_file_name("pcscope-software.exe"),
        executable.with_file_name("pcscope-powershell-adapter.exe"),
    ]
    .into_iter()
    .map(|path| Collector { path, domain: None })
    .collect();
    for domain in [
        "system",
        "security",
        "network",
        "storage",
        "startup",
        "desktop",
        "virtualization",
        "applications",
        "scoped",
    ] {
        collectors.push(Collector {
            path: executable.with_file_name("pcscope-powershell-adapter.exe"),
            domain: Some(domain.into()),
        });
    }
    report.phase("opening_history");
    let store = pcscope_storage::Store::open_program(data.join("history.db")).context("opening history database")?;
    report.phase("reading_schedule");
    let schedule = schedule::Schedule::new(
        collectors.iter().map(Collector::name).collect(),
        &store.plugin_settings()?,
    )?;
    let store = Arc::new(Mutex::new(store));
    report.phase("starting_projection");
    let projector = projection::Worker::start(store.clone(), projection::machine_identity()?)?;
    let state = Arc::new(State {
        endpoint: endpoint.clone(),
        query_slots: tokio::sync::Semaphore::new(2),
        monitoring: Mutex::new(json!(null)),
        checking: AtomicBool::new(false),
        monitoring_writes: AtomicUsize::new(0),
        data_dir: data.clone(),
        schedule: Mutex::new(schedule),
        store,
        projection_ready: projector.ready.clone(),
        projection_status: projector.status.clone(),
        collecting: AtomicBool::new(false),
        last_result: Mutex::new(json!(null)),
        collectors,
        event_collector: executable.with_file_name("pcscope-eventlog.exe"),
        polling: AtomicBool::new(false),
        stopping: AtomicBool::new(false),
        event_result: Mutex::new(json!(null)),
        usn_result: Mutex::new(json!(null)),
        usn_exclusions: ["history.db", "history.db-wal", "history.db-shm"]
            .iter()
            .map(|name| data.join(name).to_string_lossy().into_owned())
            .collect(),
        shutdown: tokio::sync::Notify::new(),
        channel_cursor: AtomicUsize::new(0),
        event_backoff: Mutex::new(event_backoff::Backoff::default()),
        check_after: Mutex::new(std::time::Instant::now()),
    });
    report.phase("recording_session");
    state
        .store
        .lock()
        .unwrap()
        .begin_session(&chrono::Utc::now().to_rfc3339()).context("recording service startup")?;
    check_monitoring(state.clone());
    let runtime_state=state.clone();
    let runtime_worker=std::thread::spawn(move ||runtime::run(runtime_state));
    let scheduled = state.clone();
    // Register before the scheduler's first enumeration so arrivals during the baseline are retained.
    let device_watcher=device_notify::Watcher::open().map_err(|error|eprintln!("device notifications unavailable; periodic state collection remains active: {error}")).ok();
    let network_watcher=device_notify::NetworkWatcher::open().map_err(|error|eprintln!("network notifications unavailable; periodic state collection remains active: {error}")).ok();
    let scheduler = tokio::spawn(async move {
        let mut device_coalescer=device_notify::Coalescer::default();
        let mut network_coalescer=device_notify::Coalescer::default();
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tick.tick().await;
            if device_coalescer.poll(device_watcher.as_ref().is_some_and(|watcher|watcher.take()),std::time::Instant::now()) {
                let mut schedule=scheduled.schedule.lock().unwrap();
                schedule.request_refresh("pcscope-powershell-adapter.exe",notification_scope::DEVICE_CORE);
                schedule.request_refresh("storage",notification_scope::DEVICE_STORAGE);
            }
            if network_coalescer.poll(network_watcher.as_ref().is_some_and(|watcher|watcher.take()),std::time::Instant::now()) {
                let mut schedule=scheduled.schedule.lock().unwrap();
                schedule.request_refresh("pcscope-powershell-adapter.exe",notification_scope::NETWORK_CORE);
                schedule.request_refresh("network",notification_scope::NETWORK_ROUTES);
            }
            let _ = start_collection(scheduled.clone(), None);
            let _ = start_event_collection(scheduled.clone(), None);
        }
    });
    let poll_state = state.clone();
    let poller = tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(10));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tick.tick().await;
            if let Ok(store) = poll_state.store.lock() {
                if let Ok(p)=pcscope_supervisor::process_identity(std::process::id()) {
                    if let Err(error)=store.observe_own_process(p.pid,p.created_ms,p.seen_ms,p.ended_ms,&p.image,"service","pcscope-service","service") {eprintln!("process observation failed: {error}");}
                }
                if let Err(error) = store.heartbeat(&chrono::Utc::now().to_rfc3339()) {
                    eprintln!("heartbeat failed: {error}");
                }
            }
        }
    });
    report.phase("ready");
    eprintln!("PCScope development user daemon ready");
    loop {
        tokio::select! {
            result=server.connect()=>{result?;},
            _=tokio::signal::ctrl_c()=>break,
            _=state.shutdown.notified()=>break,
        }
        let connected = server;
        server = pcscope_ipc::server(&endpoint, false)?;
        let state = state.clone();
        tokio::spawn(async move {
            let operation = async {
                let (input, mut output) = tokio::io::split(connected);
                let mut bytes = Vec::new();
                BufReader::new(input)
                    .take(65537)
                    .read_until(b'\n', &mut bytes)
                    .await?;
                let response = if bytes.len() > 65536 {
                    json!({"ok":false,"error":"request exceeds limit"})
                } else {
                    let result=match serde_json::from_slice::<Request>(&bytes) {
                        Ok(request)=>tokio::task::spawn_blocking(move ||dispatch(&state,request)).await?,
                        Err(error)=>Err(error.into()),
                    };
                    match result {
                        Ok(data) => json!({"ok":true,"data":data}),
                        Err(e) => json!({"ok":false,"error":e.to_string()}),
                    }
                };
                let mut encoded = serde_json::to_vec(&response)?;
                if encoded.len() + 1 > pcscope_ipc::MAX_RESPONSE {
                    encoded = serde_json::to_vec(
                        &json!({"ok":false,"error":"response too large; reduce query limit"}),
                    )?;
                }
                encoded.push(b'\n');
                output.write_all(&encoded).await?;
                Ok::<_, anyhow::Error>(())
            };
            let _ = tokio::time::timeout(Duration::from_secs(10), operation).await;
        });
    }
    report.phase("stopping");
    state.stopping.store(true, Ordering::SeqCst);
    // Stop derived work before waiting for collectors that may be queued on the same writer.
    // The worker cancels only its own SQL lease; collector transactions keep their normal path.
    projector.request_stop();
    scheduler.abort();
    poller.abort();
    while state.collecting.load(Ordering::SeqCst) || state.polling.load(Ordering::SeqCst) || state.checking.load(Ordering::SeqCst) || state.monitoring_writes.load(Ordering::SeqCst)>0 {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    runtime_worker.join().map_err(|_|anyhow::anyhow!("runtime worker panicked"))?;
    drop(projector); // Finish the current transaction; remaining work resumes next startup.
    state.store.lock().map_err(|_| anyhow::anyhow!("storage lock poisoned"))?
        .end_session(&chrono::Utc::now().to_rfc3339())?;
    Ok(())
}

#[cfg(not(windows))]
fn main() -> Result<()> {
    anyhow::bail!("Windows required")
}
