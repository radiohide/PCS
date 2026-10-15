//! Lightweight reads use committed WAL state, independently of the ingestion/projection writer.
use super::{Collector, State};
use anyhow::{Context, Result};
use pcscope_ipc::Request;
use pcscope_storage::Store;
use serde_json::{json, Value};
use std::sync::{atomic::Ordering, Mutex};
use std::time::Duration;

pub fn reader(state: &State) -> Result<Store> {
    Store::open_reader(state.data_dir.join("history.db"), Duration::from_secs(2))
}

pub fn handles(request: &Request) -> bool {
    matches!(request, Request::Status | Request::MonitoringSettings | Request::PrivacySettings |
        Request::ItemControls | Request::DirectoryWatchSettings | Request::FileWatchSettings |
        Request::RuntimeWatchSettings | Request::Retention)
}

fn snapshot(value: &Mutex<Value>) -> Result<Value> {
    Ok(value.lock().map_err(|_| anyhow::anyhow!("state lock poisoned"))?.clone())
}

pub fn dispatch(state: &State, request: &Request) -> Result<Value> {
    let store=reader(state)?;
    let mut value=match request {
        Request::Status => {
            // Clone each in-memory result before any SQL. No status mutex encloses I/O.
            let last=snapshot(&state.last_result)?;
            let events=snapshot(&state.event_result)?;
            let usn=snapshot(&state.usn_result)?;
            let projection=snapshot(&state.projection_status)?;
            json!({"mode":"development-user-daemon","data_dir":state.data_dir,
                "instance_id":state.endpoint.instance_id,"pid":std::process::id(),
                "collecting":state.collecting.load(Ordering::SeqCst),"last_collection":last,
                "collectors":state.collectors.iter().map(Collector::name).collect::<Vec<_>>(),
                "event_collection":events,"usn_collection":usn,"projection_worker":projection,
                "schedule_mode":"per-plugin-after-completion","default_event_poll_seconds":10,
                "default_schedule_seconds":300,"counts":store.counts()?})
        },
        Request::MonitoringSettings => {
            let settings=store.monitoring_settings()?;
            let report=snapshot(&state.monitoring)?;
            let history=snapshot(&state.event_result)?;
            json!({"settings":settings,"report":report,"history":history,
                "checking":state.checking.load(Ordering::SeqCst),"elevated":pcscope_ipc::elevated()?,
                "data_dir":state.data_dir,"instance_id":state.endpoint.instance_id,"pid":std::process::id()})
        },
        Request::PrivacySettings => json!({"show_sensitive":store.show_sensitive()?}),
        Request::ItemControls => store.item_controls()?,
        Request::DirectoryWatchSettings => store.directory_watch_settings()?,
        Request::FileWatchSettings => store.file_watch_settings()?,
        Request::RuntimeWatchSettings => store.runtime_watch_settings()?,
        Request::Retention => store.retention()?,
        _ => anyhow::bail!("not a lightweight read request"),
    };
    // Failure stays an error; never infer permission to expose protected values from a stale cache.
    pcscope_contracts::sensitive::present(&mut value,store.show_sensitive().context("reading current privacy setting")?);
    Ok(value)
}
