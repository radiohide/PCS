//! Production queue consumer: bounded transactions, durable restart and retry.
use anyhow::Result;
use pcscope_storage::Store;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use std::time::{Duration,Instant};

// Fixed-size, in-memory measurements distinguish writer contention from actual processing.
// No extra sampling task, database writes or growing history is needed.
#[derive(Default)]
struct StageTimings {
    batches:u64,wait_ms:u64,work_ms:u64,max_wait_ms:u64,max_work_ms:u64,
    last_wait_ms:u64,last_work_ms:u64,
}
impl StageTimings {
    fn run<T>(&mut self,store:&Arc<Mutex<Store>>,action:impl FnOnce(&Store)->Result<T>)->Result<T> {
        let started=Instant::now();
        let guard=store.lock().map_err(|_|anyhow::anyhow!("storage lock poisoned"))?;
        let acquired=Instant::now();let result=action(&guard);let finished=Instant::now();drop(guard);
        self.last_wait_ms=acquired.duration_since(started).as_millis().try_into().unwrap_or(u64::MAX);
        self.last_work_ms=finished.duration_since(acquired).as_millis().try_into().unwrap_or(u64::MAX);
        self.batches=self.batches.saturating_add(1);self.wait_ms=self.wait_ms.saturating_add(self.last_wait_ms);
        self.work_ms=self.work_ms.saturating_add(self.last_work_ms);
        self.max_wait_ms=self.max_wait_ms.max(self.last_wait_ms);self.max_work_ms=self.max_work_ms.max(self.last_work_ms);result
    }
    fn view(&self)->Value {
        json!({"batches":self.batches,"wait_ms_total":self.wait_ms,"work_ms_total":self.work_ms,
            "max_wait_ms":self.max_wait_ms,"max_work_ms":self.max_work_ms,
            "last_wait_ms":self.last_wait_ms,"last_work_ms":self.last_work_ms})
    }
}

pub struct Worker {
    pub ready: Arc<AtomicBool>,
    pub status: Arc<Mutex<Value>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Worker {
    pub fn start(store: Arc<Mutex<Store>>, machine: String) -> Result<Self> {
        let installation = store.lock().unwrap().projection_installation(&machine)?;
        let ready = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let status = Arc::new(Mutex::new(json!({"state":"catching_up"})));
        let (worker_ready, worker_stop, worker_status) = (ready.clone(), stop.clone(), status.clone());
        let thread = std::thread::Builder::new().name("object-projection".into()).spawn(move || {
            let mut display:std::result::Result<Value,String>=Ok(json!({"pending":true}));
            let mut display_retry_at=Instant::now();let mut display_failures=0u32;
            let mut projection_timings=StageTimings::default();let mut display_timings=StageTimings::default();
            let mut baseline_timings=StageTimings::default();
            let mut baselines:std::result::Result<Value,String>=Ok(json!({"pending":true}));
            let mut baseline_retry_at=Instant::now();let mut baseline_failures=0u32;
            let mut association_timings=StageTimings::default();
            let mut associations:std::result::Result<Value,String>=Ok(json!({"pending":true}));
            let mut association_retry_at=Instant::now();let mut association_failures=0u32;
            while !worker_stop.load(Ordering::SeqCst) {
                // Release the shared writer after every small atomic batch.
                let result = projection_timings.run(&store,|store|store.project_pending(&machine, &installation, 64));
                // Reacquire after raw projection: collectors get a chance to commit between
                // the two bounded transactions. Display work cannot be starved by raw backlog.
                std::thread::yield_now();
                let mut baseline_queued=false;
                // Snapshot observations and their raw records commit together. Appending
                // inventory work to the same FIFO keeps those observations ahead of it,
                // without waiting for a continuously fed collection queue to become empty.
                if result.is_ok()&&Instant::now()>=baseline_retry_at {
                    baselines=baseline_timings.run(&store,|s|s.prepare_snapshot_baselines(32)).map_err(|e|e.to_string());
                    baseline_queued=baselines.as_ref().ok().is_some_and(|r|r["queued"].as_u64().unwrap_or(0)>0);
                    if baselines.is_err(){baseline_failures=baseline_failures.saturating_add(1);}
                    else{baseline_failures=0;}
                    baseline_retry_at=Instant::now()+Duration::from_secs(if baseline_failures>0{(1u64<<baseline_failures.min(6)).min(60)}else{1});
                    std::thread::yield_now();
                }
                if Instant::now()>=display_retry_at {
                    display=display_timings.run(&store,|store|store.prepare_display(128,Duration::from_millis(40))).map_err(|error|error.to_string());
                    if display.is_err() {
                        display_failures=display_failures.saturating_add(1);
                        display_retry_at=Instant::now()+Duration::from_secs((1u64<<display_failures.min(6)).min(60));
                    }else{display_failures=0;display_retry_at=Instant::now();}
                }
                if Instant::now()>=association_retry_at && associations.as_ref().map_or(true,|v|v["pending"]==true) {
                    std::thread::yield_now();
                    associations=association_timings.run(&store,|s|s.repair_event_associations(512,Duration::from_millis(40))).map_err(|e|e.to_string());
                    if associations.is_err(){association_failures=association_failures.saturating_add(1)}else{association_failures=0}
                    association_retry_at=Instant::now()+if association_failures>0 {Duration::from_secs((1u64<<association_failures.min(6)).min(60))}else{Duration::from_millis(250)};
                }
                let delay = match result {
                    Ok(report) => {
                        if report.examined == 0 && !baseline_queued { worker_ready.store(true, Ordering::SeqCst); }
                        *worker_status.lock().unwrap() = json!({"state":if report.examined==0 {"idle"} else {"projecting"},
                            "examined":report.examined,"projected":report.projected,
                            "rejected":report.rejected,"unadapted":report.unadapted,
                            "display":display.as_ref().ok(),"display_error":display.as_ref().err().map(ToString::to_string),
                            "initial_inventory":baselines.as_ref().ok(),"initial_inventory_error":baselines.as_ref().err(),
                            "association_repair":associations.as_ref().ok(),"association_repair_error":associations.as_ref().err(),
                            "timing":{"projection":projection_timings.view(),"display":display_timings.view(),"initial_inventory":baseline_timings.view(),"association_repair":association_timings.view()},
                            "at":chrono::Utc::now().to_rfc3339()});
                        if report.examined == 0 {
                            if display.as_ref().ok().is_some_and(|d|d["history_pending"]==true||d["built"].as_u64().unwrap_or(0)>0||d["linked"].as_u64().unwrap_or(0)>0) {Duration::from_millis(10)}else{Duration::from_millis(250)}
                        } else {Duration::from_millis(10)}
                    }
                    Err(error) => {
                        // Transaction rollback leaves the evidence and pending work intact.
                        worker_ready.store(false, Ordering::SeqCst);
                        *worker_status.lock().unwrap() = json!({"state":"retrying","error":error.to_string(),"at":chrono::Utc::now().to_rfc3339(),
                            "timing":{"projection":projection_timings.view(),"display":display_timings.view()}});
                        eprintln!("object projection failed; pending work retained: {error:#}");
                        Duration::from_secs(1)
                    }
                };
                std::thread::park_timeout(delay);
            }
        })?;
        Ok(Self {ready, status, stop, thread: Some(thread)})
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            thread.thread().unpark();
            let _ = thread.join();
        }
    }
}

#[cfg(windows)]
pub fn machine_identity() -> Result<String> {
    use winreg::{enums::*, RegKey};
    // Match the snapshot collectors' registry view and identity spelling.
    Ok(RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(r"SOFTWARE\Microsoft\Cryptography", KEY_READ | KEY_WOW64_64KEY)?
        .get_value("MachineGuid")?)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn material(run: usize) -> pcscope_contracts::Envelope {
        pcscope_contracts::parse(&json!({"protocol_version":1,"request_id":format!("run-{run}"),
            "type":"snapshot","source":"pcscope.startup.tasks","scope":"machine:M/user:S/schema:1/context:/shard:0",
            "run_id":format!("run-{run}"),"observed_at":"2026-09-12T01:00:00Z","complete":true,
            "items":[{"id":"\\task","data":{"path":"\\","name":"Task","enabled":true}}]}).to_string()).unwrap()
    }
    fn count(store: &Arc<Mutex<Store>>, table: &str) -> i64 {
        store.lock().unwrap().reader().query_row(&format!("SELECT count(*) FROM {table}"), [], |r|r.get(0)).unwrap()
    }
    fn wait_for(mut predicate: impl FnMut() -> bool) {
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while !predicate() {
            assert!(std::time::Instant::now() < deadline, "production worker did not converge");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    #[test]
    fn rejected_material_stays_stored_and_query_reports_the_failure() {
        let store = Arc::new(Mutex::new(Store::open(":memory:").unwrap()));
        let mut input = material(0);
        if let pcscope_contracts::Message::Snapshot {source, ..} = &mut input.message { *source="pcscope.unknown-source".into(); }
        store.lock().unwrap().ingest(&input).unwrap();
        let pending = store.lock().unwrap().query(&pcscope_contracts::query::QueryRequest::ProjectionStatus).unwrap();
        assert!(pending.data.unwrap().as_array().unwrap().iter().any(|row|row["projection"]=="objects" && row["pending"]==1));
        let worker = Worker::start(store.clone(), "M".into()).unwrap();
        wait_for(|| worker.ready.load(Ordering::SeqCst));
        assert_eq!(count(&store, "evidence"), 1);
        assert_eq!(count(&store, "projection_queue WHERE state='failed'"), 1);
        let failed = store.lock().unwrap().query(&pcscope_contracts::query::QueryRequest::ProjectionStatus).unwrap();
        assert!(failed.data.unwrap().as_array().unwrap().iter().any(|row|row["projection"]=="objects" && row["failed"]==1 && row["last_error"].is_string()));
    }

    #[test]
    fn failed_transaction_is_retried_without_losing_evidence() {
        let store = Arc::new(Mutex::new(Store::open(":memory:").unwrap()));
        store.lock().unwrap().ingest(&material(0)).unwrap();
        store.lock().unwrap().reader().execute_batch("CREATE TRIGGER injected_failure BEFORE INSERT ON object_instances BEGIN SELECT RAISE(ABORT,'injected disk failure'); END;").unwrap();
        let worker = Worker::start(store.clone(), "M".into()).unwrap();
        wait_for(|| worker.status.lock().unwrap()["state"] == "retrying");
        assert!(!worker.ready.load(Ordering::SeqCst));
        assert_eq!(count(&store, "evidence"), 1);
        assert_eq!(count(&store, "projection_queue WHERE state='pending'"), 1);
        store.lock().unwrap().reader().execute_batch("DROP TRIGGER injected_failure").unwrap();
        wait_for(|| worker.ready.load(Ordering::SeqCst));
        assert_eq!(count(&store, "object_instances"), 1);
        assert_eq!(count(&store, "evidence"), 2,"snapshot and initial-inventory reference both survive retry");
    }

    #[test]
    fn reopening_database_keeps_identity_and_pending_work() {
        let path = std::env::temp_dir().join(format!("pcscope-projection-{}-{}.db", std::process::id(), chrono::Utc::now().timestamp_nanos_opt().unwrap()));
        let installation;
        {
            let mut store = Store::open(&path).unwrap();
            installation = store.projection_installation("M").unwrap();
            store.ingest(&material(0)).unwrap();
            // Simulate an existing schema-8 database from before the query indexes.
            store.reader().execute_batch("DROP INDEX evidence_source_address; DROP INDEX projection_queue_address_state").unwrap();
        }
        {
            let store = Arc::new(Mutex::new(Store::open(&path).unwrap()));
            assert_eq!(count(&store, "sqlite_master WHERE type='index' AND name IN ('evidence_source_address','projection_queue_address_state')"),2);
            let worker = Worker::start(store.clone(), "M".into()).unwrap();
            wait_for(|| worker.ready.load(Ordering::SeqCst));
            assert_eq!(installation, store.lock().unwrap().projection_installation("M").unwrap());
            assert_eq!(count(&store, "object_instances"), 1);
            assert_eq!(count(&store, "projection_queue WHERE state='pending'"), 0);
        }
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn production_worker_drains_startup_and_live_work_and_resumes_without_duplicates() {
        let store = Arc::new(Mutex::new(Store::open(":memory:").unwrap()));
        for run in 0..130 {store.lock().unwrap().ingest(&material(run)).unwrap();}
        let worker = Worker::start(store.clone(), "M".into()).unwrap();
        wait_for(|| worker.ready.load(Ordering::SeqCst));
        assert_eq!(count(&store, "projection_queue WHERE state='pending'"), 0);
        assert_eq!(count(&store, "object_instances"), 1);
        store.lock().unwrap().ingest(&material(131)).unwrap();
        wait_for(|| count(&store, "projection_queue WHERE state='pending'")==0);
        drop(worker);
        let identity = store.lock().unwrap().projection_installation("M").unwrap();
        store.lock().unwrap().ingest(&material(132)).unwrap();
        let worker = Worker::start(store.clone(), "M".into()).unwrap();
        wait_for(|| worker.ready.load(Ordering::SeqCst));
        assert_eq!(identity, store.lock().unwrap().projection_installation("M").unwrap());
        assert_eq!(count(&store, "object_instances"), 1);
        assert_eq!(count(&store, "evidence"), 133,"one retained initial-inventory record is prepared once");
        assert_eq!(count(&store, "object_actions WHERE action_id='observed.first_seen' AND json_extract(source_record,'$.snapshot_presence.baseline')=1"),1);
        assert!(store.lock().unwrap().projection_installation("other-machine").is_err());
    }
}
