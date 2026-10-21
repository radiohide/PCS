use super::*;
use std::sync::mpsc;

fn fixture() -> Arc<State> {
    let data_dir=std::env::temp_dir().join(format!("pcscope-read-api-{}-{}",std::process::id(),chrono::Utc::now().timestamp_nanos_opt().unwrap()));
    std::fs::create_dir_all(&data_dir).unwrap();
    let endpoint=pcscope_ipc::Endpoint::for_directory(&data_dir).unwrap();
    let mut store=pcscope_storage::Store::open_program(data_dir.join("history.db")).unwrap();
    store.set_monitoring_settings(&json!({"configured":true,"mode":"enhanced","repair":false}),"2026-10-13T00:00:00Z").unwrap();
    store.set_show_sensitive(false).unwrap();
    Arc::new(State {
        endpoint, data_dir, store:Arc::new(Mutex::new(store)),
        query_slots:tokio::sync::Semaphore::new(2), monitoring:Mutex::new(json!({"checked":true})),
        checking:AtomicBool::new(false),monitoring_writes:AtomicUsize::new(0),schedule:Mutex::new(schedule::Schedule::new(vec![],&json!({})).unwrap()),
        projection_ready:Arc::new(AtomicBool::new(true)),projection_status:Arc::new(Mutex::new(json!({"state":"idle"}))),
        collecting:AtomicBool::new(false),last_result:Mutex::new(json!(null)),collectors:vec![],
        event_collector:PathBuf::new(),polling:AtomicBool::new(false),stopping:AtomicBool::new(false),
        event_result:Mutex::new(json!({"channels":{}})),usn_result:Mutex::new(json!(null)),usn_exclusions:vec![],
        shutdown:tokio::sync::Notify::new(),channel_cursor:AtomicUsize::new(0),
        event_backoff:Mutex::new(event_backoff::Backoff::default()),check_after:Mutex::new(std::time::Instant::now()),
    })
}

#[test]
fn collection_period_writes_are_persisted_atomically_and_patch_only_requested_fields() {
    let state=fixture();
    *state.schedule.lock().unwrap()=schedule::Schedule::new(vec!["scoped".into(),"network".into()],&json!({})).unwrap();
    dispatch(&state,Request::SetPlugin{id:"network".into(),enabled:Some(false),seconds:None}).unwrap();
    dispatch(&state,Request::SetGlobalCollectionSchedule{seconds:60}).unwrap();
    dispatch(&state,Request::SetPlugin{id:"scoped".into(),enabled:None,seconds:Some(120)}).unwrap();
    dispatch(&state,Request::SetPlugin{id:"network".into(),enabled:Some(true),seconds:None}).unwrap();
    let saved=state.store.lock().unwrap().plugin_settings().unwrap();
    let restored=schedule::Schedule::new(vec!["scoped".into(),"network".into()],&saved).unwrap();
    assert_eq!(restored.config(),state.schedule.lock().unwrap().config());
    assert_eq!(saved["scoped"]["seconds"],120);
    assert_eq!(saved["network"]["seconds"],60);
    assert_eq!(saved["_runtime_seconds"],120);
    assert_eq!(dispatch(&state,Request::GlobalCollectionSchedule).unwrap()["seconds"],60);
    // A disk/write rejection cannot leave half of the jobs changed in memory or on disk.
    state.store.lock().unwrap().reader().execute_batch("CREATE TEMP TRIGGER reject_schedule BEFORE UPDATE ON settings WHEN NEW.key='plugin_schedule' BEGIN SELECT RAISE(ABORT,'test write failure'); END;").unwrap();
    assert!(dispatch(&state,Request::SetGlobalCollectionSchedule{seconds:90}).is_err());
    assert_eq!(state.schedule.lock().unwrap().config(),saved);
    assert_eq!(state.store.lock().unwrap().plugin_settings().unwrap(),saved);
    state.store.lock().unwrap().reader().execute_batch("DROP TRIGGER reject_schedule;").unwrap();
    dispatch(&state,Request::SetGlobalCollectionSchedule{seconds:60}).unwrap();
    assert_eq!(state.store.lock().unwrap().plugin_settings().unwrap()["scoped"]["seconds"],60);
}

#[test]
fn settings_status_and_plugins_read_committed_values_while_writer_is_busy() {
    let state=fixture();
    let writer=state.store.lock().unwrap();
    writer.reader().execute_batch("BEGIN IMMEDIATE; UPDATE settings SET value='true' WHERE key='show_sensitive'; UPDATE settings SET value='{\"configured\":true,\"mode\":\"low\",\"repair\":false}' WHERE key='monitoring';").unwrap();
    let (send,receive)=mpsc::channel(); let client=state.clone();
    let thread=std::thread::spawn(move || {
        let requests=[Request::Status,Request::MonitoringSettings,Request::PrivacySettings,Request::FileWatchSettings,
            Request::DirectoryWatchSettings,Request::RuntimeWatchSettings,Request::ItemControls,Request::Plugins,Request::Retention,Request::GlobalCollectionSchedule];
        send.send(requests.into_iter().map(|request|dispatch(&client,request)).collect::<Result<Vec<_>>>()).unwrap();
    });
    let result=receive.recv_timeout(Duration::from_secs(2));
    writer.reader().execute_batch("ROLLBACK;").unwrap();drop(writer);thread.join().unwrap();
    let values=result.expect("read-only APIs waited for the global writer").unwrap();
    assert_eq!(values[0]["counts"]["records"],1);
    assert_eq!(values[0]["projection_worker"]["state"],"idle");
    assert_eq!(values[1]["settings"]["mode"],"enhanced","an uncommitted setting must not leak");
    assert_eq!(values[1]["settings"]["repair"],false);
    assert_eq!(values[2]["show_sensitive"],false);
    assert!(values[6]["items"].is_array());assert!(values[7].is_array());
    assert_eq!(values[8]["mode"],"unlimited");
    assert!(values[8]["physical_bytes"].as_u64().unwrap()>0);
}

#[test]
fn shutdown_acknowledgment_does_not_wait_for_writer_or_privacy_read() {
    let state=fixture();let writer=state.store.lock().unwrap();
    let (send,receive)=mpsc::channel();let client=state.clone();
    let thread=std::thread::spawn(move ||send.send(dispatch(&client,Request::Shutdown)).unwrap());
    let result=receive.recv_timeout(Duration::from_secs(1));drop(writer);thread.join().unwrap();
    assert_eq!(result.expect("shutdown reply waited for writer").unwrap()["stopping"],true);
    assert!(state.stopping.load(Ordering::SeqCst));
}

#[test]
fn model_refresh_revision_only_exposes_committed_changes() {
    let state=fixture();
    let before=dispatch(&state,Request::Status).unwrap()["read_model_revision"].clone();
    let writer=state.store.lock().unwrap();
    writer.reader().execute_batch("BEGIN IMMEDIATE;
        INSERT INTO object_actions(unassigned_reason,action_id,result,precision,observed_at,rule_version)
        VALUES('test','execution.process_started','unknown','observed','2026-10-19T00:00:00Z','test');").unwrap();
    assert_eq!(dispatch(&state,Request::Status).unwrap()["read_model_revision"],before);
    writer.reader().execute_batch("COMMIT;").unwrap();drop(writer);
    let after=dispatch(&state,Request::Status).unwrap()["read_model_revision"].clone();
    assert_ne!(after,before);
    assert_eq!(dispatch(&state,Request::Status).unwrap()["read_model_revision"],after);
}

#[test]
fn completed_monitoring_is_visible_while_history_write_remains_pending() {
    let state=fixture();let writer=state.store.lock().unwrap();
    state.checking.store(true,Ordering::SeqCst);
    let worker=state.clone();let thread=std::thread::spawn(move ||finish_monitoring(&worker,json!({"completed":true})));
    let deadline=std::time::Instant::now()+Duration::from_secs(1);
    while state.checking.load(Ordering::SeqCst) && std::time::Instant::now()<deadline {std::thread::yield_now();}
    let published=!state.checking.load(Ordering::SeqCst);
    let pending=state.monitoring_writes.load(Ordering::SeqCst);
    let value=dispatch(&state,Request::MonitoringSettings);
    drop(writer);thread.join().unwrap();
    assert!(published,"completed check waited for storage before publishing");
    assert_eq!(pending,1,"graceful shutdown must still wait for the pending history write");
    assert_eq!(value.unwrap()["report"]["completed"],true);
    assert_eq!(state.monitoring_writes.load(Ordering::SeqCst),0);
}

#[test]
fn status_and_monitoring_apply_current_privacy_without_mutating_cached_material() {
    let state=fixture();
    let original=json!("test-only-protected-value");
    let protected=pcscope_contracts::sensitive::protect(&original).unwrap();
    *state.last_result.lock().unwrap()=json!({"value":protected.clone()});
    *state.monitoring.lock().unwrap()=json!({"value":protected.clone()});
    for reveal in [false,true,false] {
        state.store.lock().unwrap().set_show_sensitive(reveal).unwrap();
        let expected=if reveal {original.clone()} else {json!(pcscope_contracts::sensitive::MASK)};
        let status=dispatch(&state,Request::Status).unwrap();
        let monitoring=dispatch(&state,Request::MonitoringSettings).unwrap();
        assert_eq!(status["last_collection"]["value"],expected);
        assert_eq!(monitoring["report"]["value"],expected);
        assert!(!status.to_string().contains("ciphertext"));
    }
    assert_eq!(state.last_result.lock().unwrap()["value"],protected);
    assert_eq!(state.monitoring.lock().unwrap()["value"],protected);
}

#[tokio::test]
async fn replacement_sends_shutdown_without_status_and_waits_for_first_pipe_owner() {
    let state=fixture();let endpoint=state.endpoint.clone();
    let server=pcscope_ipc::server(&endpoint,true).unwrap();
    let (release,wait_release)=tokio::sync::oneshot::channel();
    let old=tokio::spawn(async move {
        server.connect().await.unwrap();let (input,mut output)=tokio::io::split(server);
        let mut bytes=vec![];BufReader::new(input).read_until(b'\n',&mut bytes).await.unwrap();
        assert!(matches!(serde_json::from_slice::<Request>(&bytes).unwrap(),Request::Shutdown));
        output.write_all(b"{\"ok\":true,\"data\":{\"stopping\":true}}\n").await.unwrap();
        wait_release.await.unwrap();
    });
    assert!(replacement::request_shutdown(&endpoint).await.unwrap());
    let error=pcscope_ipc::server(&endpoint,true).unwrap_err();
    assert!(replacement::pipe_still_owned(&error),"first pipe ownership was not classified as occupied: {error}");
    release.send(()).unwrap();old.await.unwrap();
    drop(pcscope_ipc::server(&endpoint,true).expect("pipe did not release after graceful owner exit"));
    assert!(!replacement::request_shutdown(&endpoint).await.unwrap(),"a missing endpoint must not authorize retrying a fresh access denial");
}

#[tokio::test]
async fn replacement_with_lost_reply_still_waits_for_selected_pipe_owner() {
    let state=fixture();let endpoint=state.endpoint.clone();
    let server=pcscope_ipc::server(&endpoint,true).unwrap();
    let old_endpoint=endpoint.clone();
    let old=tokio::spawn(async move {
        server.connect().await.unwrap();let mut bytes=vec![];
        let mut input=BufReader::new(server);
        input.read_until(b'\n',&mut bytes).await.unwrap();
        assert!(matches!(serde_json::from_slice::<Request>(&bytes).unwrap(),Request::Shutdown));
        // An older daemon may close its request connection before replying while its listening
        // instance remains alive until collection finishes. Dropping this connection sends EOF.
        let listener=pcscope_ipc::server(&old_endpoint,false).unwrap();
        drop(input);listener
    });
    assert!(replacement::request_shutdown(&endpoint).await.unwrap());let old_listener=old.await.unwrap();
    assert!(replacement::pipe_still_owned(&pcscope_ipc::server(&endpoint,true).unwrap_err()));
    drop(old_listener);
    drop(pcscope_ipc::server(&endpoint,true).expect("must acquire ownership only after old listener exits"));
}
