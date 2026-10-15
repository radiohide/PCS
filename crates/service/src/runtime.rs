//! SIHost preset: successful native module material derives a bounded automatic file watch set.
//! UI, stored events and rules never schedule collection. This worker owns its fixed deadlines.
#[path="runtime_notify.rs"] mod notify;
use super::{State,collect_owned,persist_collector_failure};
use anyhow::{ensure,Result};
use pcscope_contracts::{Envelope,Message,Item};
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use std::{collections::{BTreeMap,BTreeSet},path::Path,sync::Arc,time::{Duration,Instant}};

pub const SOURCES:[&str;3]=["pcscope.execution.modules","pcscope.execution.module-scan-status","pcscope.execution.module-file-state"];
const FILE_LIMIT:usize=4096;const DIRECTORY_LIMIT:usize=128;
#[derive(Clone,Serialize,Deserialize)]struct Binding {
    pid:u32,process_start:String,process_name:String,module_base:String,first_observed:String,last_observed:String,
    #[serde(default)]module_path:String,
}
#[derive(Clone,Serialize,Deserialize)]struct FileEntry {
    path:String,bindings:BTreeMap<String,Binding>,first_baseline:Option<Value>,last_valid:Option<Value>,last_attempt:Option<Value>,last_integrity:Option<String>,
    #[serde(default)]failures:u32,
    #[serde(default)]retry_at:Option<String>,
}
#[derive(Default,Serialize,Deserialize)]struct Saved {
    #[serde(default)]files:BTreeMap<String,FileEntry>,
    #[serde(default)]revision:u64,
    #[serde(default="state_version")]format_version:u32,
}
fn state_version()->u32{1}
struct Pending {first:Instant,last:Instant,reason:String,integrity:bool,not_before:Instant}
struct Manager {
    saved:Saved,pending:BTreeMap<String,Pending>,watchers:BTreeMap<String,notify::Watcher>,failed_dirs:BTreeMap<String,Instant>,failures:BTreeMap<String,u32>,partial:bool,
    rename_old:BTreeMap<String,(String,Instant)>,move_candidates:BTreeMap<String,FileEntry>,
}
fn path_key(path:&str)->String {path.to_lowercase()}
fn supported_path(path:&str,internal:&Path)->bool {
    let bytes=path.as_bytes();
    if bytes.len()<4||!bytes[0].is_ascii_alphabetic()||bytes[1]!=b':'||bytes[2]!=b'\\'||path[3..].contains([':', '*','?','<','>','|','"']) {return false;}
    let own=internal.to_string_lossy().trim_start_matches(r"\\?\").trim_end_matches('\\').to_lowercase();
    let key=path_key(path);!(key==own||key.starts_with(&(own+"\\")))
}
impl Manager {
    fn new(saved:Value)->Result<Self> {
        let saved:Saved=serde_json::from_value(saved)?;
        ensure!(saved.format_version==1,"unsupported runtime watch state version; existing state retained");
        ensure!(saved.files.len()<=FILE_LIMIT,"persisted runtime file set exceeds limit");
        ensure!(saved.files.values().all(|f|f.bindings.len()<=64),"persisted module relation set exceeds limit");
        Ok(Self{saved,pending:BTreeMap::new(),watchers:BTreeMap::new(),failed_dirs:BTreeMap::new(),failures:BTreeMap::new(),partial:false,rename_old:BTreeMap::new(),move_candidates:BTreeMap::new()})
    }
    fn queue(&mut self,key:&str,reason:&str,integrity:bool,immediate:bool) {
        if !self.saved.files.contains_key(key)&&!self.move_candidates.contains_key(key){return;}
        let now=Instant::now();
        let persisted_retry=self.saved.files.get(key).and_then(|f|f.retry_at.as_deref()).and_then(|s|chrono::DateTime::parse_from_rfc3339(s).ok()).map(|time|(time.with_timezone(&chrono::Utc)-chrono::Utc::now()).num_milliseconds().max(0) as u64).unwrap_or(0);
        self.pending.entry(key.into()).and_modify(|p|{p.last=now;p.integrity|=integrity;p.reason=reason.into();}).or_insert(Pending{first:now,last:now,reason:reason.into(),integrity,not_before:now+Duration::from_millis(persisted_retry.max(if immediate{0}else{1000}))});
    }
    fn observe(&mut self,messages:&[Envelope],internal:&Path)->bool {
        let mut changed=false;
        for message in messages {
            let Message::Snapshot{source,complete:true,items,observed_at,..}=&message.message else{continue};
            if source!=SOURCES[0]{continue;}
            for item in items {
                let p=&item.data;
                let (Some(path),Some(pid),Some(start),Some(name))=(p["module_path"].as_str(),p["pid"].as_u64(),p["process_start"].as_str(),p["process_name"].as_str()) else{self.partial=true;continue};
                if !supported_path(path,internal){self.partial=true;continue;}
                let key=path_key(path);
                if !self.saved.files.contains_key(&key) {
                    if self.saved.files.len()>=FILE_LIMIT {self.partial=true;continue;}
                    self.saved.files.insert(key.clone(),FileEntry{path:path.into(),bindings:BTreeMap::new(),first_baseline:None,last_valid:None,last_attempt:None,last_integrity:None,failures:0,retry_at:None});
                    self.saved.revision+=1;self.queue(&key,"first_module_observation",true,true);
                }
                let file=self.saved.files.get_mut(&key).unwrap();
                let process_key=format!("{pid}/{start}");
                if let Some(binding)=file.bindings.get_mut(&process_key){binding.last_observed=observed_at.clone();binding.module_base=p["module_base"].as_str().unwrap_or("").into();}
                else {
                    if file.bindings.len()>=64 {self.partial=true;continue;}
                    file.bindings.insert(process_key,Binding{pid:pid as u32,process_start:start.into(),process_name:name.into(),module_base:p["module_base"].as_str().unwrap_or("").into(),first_observed:observed_at.clone(),last_observed:observed_at.clone(),module_path:path.into()});
                }
                changed=true;
            }
        }
        changed
    }
    fn eligible(&self,key:&str,names:&BTreeSet<String>,internal:&Path)->bool {
        self.saved.files.get(key).or_else(||self.move_candidates.get(key)).is_some_and(|f|supported_path(&f.path,internal)&&f.bindings.values().any(|b|names.contains(&b.process_name)))
    }
    fn directories(&mut self,names:&BTreeSet<String>,internal:&Path)->Vec<String> {
        let mut parents:BTreeSet<String>=BTreeSet::new();
        for (key,file) in &self.saved.files {if self.eligible(key,names,internal){if let Some(parent)=Path::new(&file.path).parent(){parents.insert(parent.to_string_lossy().to_lowercase());}}}
        self.watchers.retain(|p,_|parents.contains(p));
        if parents.len()>DIRECTORY_LIMIT {self.partial=true;}
        parents.into_iter().take(DIRECTORY_LIMIT).collect()
    }
    fn notification_queue(&mut self)->Vec<String> {
        let mut gaps=Vec::new();let mut changed=Vec::new();let mut failed=Vec::new();let mut candidates=Vec::new();
        for (directory,watcher) in &mut self.watchers {
            match watcher.poll() {
                Ok(Some(rows)) if rows.is_empty()=>{gaps.push(format!("directory notification overflow: {directory}; only known files will be rescanned"));for (key,file) in &self.saved.files{if Path::new(&file.path).parent().is_some_and(|p|p.to_string_lossy().to_lowercase()==*directory){changed.push((key.clone(),"notification_overflow".into()));}}},
                Ok(Some(rows))=>for row in rows {
                    let path=Path::new(directory).join(row.name).to_string_lossy().into_owned();let key=path_key(&path);
                    if row.action==4&&self.saved.files.contains_key(&key){self.rename_old.insert(directory.clone(),(key.clone(),Instant::now()));}
                    if row.action==5 {if let Some((old,when))=self.rename_old.remove(directory){if when.elapsed()<=Duration::from_secs(5)&&old!=key&&!self.saved.files.contains_key(&key){if let Some(previous)=self.saved.files.get(&old){let mut candidate=previous.clone();candidate.path=path;candidates.push((key.clone(),candidate));}}}}
                    if self.saved.files.contains_key(&key){changed.push((key,format!("native_notification/action:{}",row.action)));}
                },
                Ok(None)=>{},
                Err(error)=>{gaps.push(format!("directory notification interrupted: {directory}: {error}"));failed.push(directory.clone());}
            }
        }
        for directory in failed {self.watchers.remove(&directory);self.failed_dirs.insert(directory,Instant::now()+Duration::from_secs(60));}
        self.rename_old.retain(|_,(_,when)|when.elapsed()<=Duration::from_secs(5));
        for (key,candidate) in candidates {
            if self.move_candidates.len()>=64||self.saved.files.len()>=FILE_LIMIT{self.partial=true;gaps.push("rename candidate budget exceeded; relocation remains unverified".into());continue;}
            self.move_candidates.insert(key.clone(),candidate);self.queue(&key,"rename_candidate; identity must match before binding",true,false);
        }
        for (key,reason) in changed {self.queue(&key,&reason,true,false);}
        gaps
    }
    fn due(&self,names:&BTreeSet<String>,internal:&Path)->Option<String> {
        let now=Instant::now();
        self.pending.iter().filter(|(k,p)|self.eligible(k,names,internal)&&p.not_before<=now&&(!p.reason.starts_with("native_notification")||now.duration_since(p.last)>=Duration::from_secs(1)||now.duration_since(p.first)>=Duration::from_secs(5)))
            .min_by_key(|(k,p)|(self.saved.files.get(*k).and_then(|f|f.last_valid.as_ref()).is_some_and(|v|v["module_origin"]=="microsoft_signed"),p.first)).map(|(k,_)|k.clone())
    }
}
fn differences(before:Option<&Value>,after:&Value)->Vec<String> {
    let Some(before)=before else{return vec!["baseline_observed".into()]};
    let mut differences=Vec::new();
    for field in ["exists","volume_identity","file_id","size","last_write_utc","attributes","owner","dacl_sddl","version","sha256","signature_status","signature_code","publisher","pe_status","pe_machine"] {
        // A lightweight observation cannot erase a previous content/signature verification.
        if after[field].is_null()||after[field]=="not_requested"{continue;}
        if before[field]!=after[field] {differences.push(field.into());}
    }
    differences
}
fn summary(state:&State,manager:&Manager,enabled:bool,file_enabled:bool,reason:&str)->Result<()> {
    let now=chrono::Utc::now().to_rfc3339();
    let unavailable=manager.saved.files.values().filter(|f|f.last_attempt.as_ref().is_none_or(|a|a["status"]=="unavailable")).count();
    let envelope=Envelope{protocol_version:1,request_id:format!("runtime-summary/{now}"),message:Message::Snapshot{source:SOURCES[1].into(),scope:format!("runtime-controller/{}",state.endpoint.instance_id),run_id:now.clone(),observed_at:now,complete:true,items:vec![Item{id:"runtime-controller".into(),data:json!({"enabled":enabled,"file_monitoring_enabled":file_enabled,"reason":reason,"tracked_files":manager.saved.files.len(),"watched_directories":manager.watchers.len(),"pending_files":manager.pending.len(),"unavailable_files":unavailable,"partial":manager.partial||unavailable>0||!manager.pending.is_empty(),"auto_watch_revision":manager.saved.revision,"metadata_seconds":300,"integrity_seconds":3600,"module_seconds":60,"notification_filter":notify::FILTER,"history_scope":"previously observed module files; next-load dependency and memory version unknown","usn":"not yet integrated; notifications and bounded periodic rescans only"})}]} };
    state.store.lock().map_err(|_|anyhow::anyhow!("storage lock poisoned"))?.ingest(&envelope)?;Ok(())
}
fn work(state:Arc<State>)->Result<()> {
    let saved=state.store.lock().unwrap().runtime_watch_state()?;
    let mut manager=Manager::new(saved)?;
    let mut revision=Value::Null;let mut active=false;
    let mut modules_due=Instant::now();let mut poll_due=Instant::now();let mut integrity_due=Instant::now()+Duration::from_secs(3600);let mut summary_due=Instant::now();
    let mut module_failures=0u32;
    while !state.stopping.load(std::sync::atomic::Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(200));
        let (config,selection,item_revision)={let store=state.store.lock().unwrap();(store.runtime_watch_settings()?,store.collection_selection("scoped")?,store.item_settings()?["revision"].clone())};
        let job_enabled=state.schedule.lock().unwrap().jobs.iter().any(|j|j.id=="scoped"&&j.enabled);
        let names:BTreeSet<String>=config["module_process_names"].as_array().context("runtime target names missing")?.iter().filter_map(|n|n.as_str().map(str::to_owned)).collect();
        let disabled=selection["disabled_sources"].as_array().context("runtime source selection missing")?;
        let enabled=config["enabled"]==true&&!names.is_empty()&&job_enabled&&!disabled.contains(&json!(SOURCES[0]));
        let file_enabled=enabled&&!disabled.contains(&json!(SOURCES[2]));
        let current_revision=json!([config["revision"],item_revision,enabled,file_enabled]);
        if revision!=current_revision {
            revision=current_revision;manager.watchers.clear();manager.pending.clear();manager.failed_dirs.clear();manager.move_candidates.clear();manager.rename_old.clear();active=enabled;modules_due=Instant::now();poll_due=Instant::now();integrity_due=Instant::now()+Duration::from_secs(3600);summary_due=Instant::now();
            summary(&state,&manager,active,file_enabled,if active{"monitoring scope started or revised; gap before this observation remains unknown"}else{"monitoring stopped by configuration/source/plugin; history retained"})?;
        }
        if !active||!state.projection_ready.load(std::sync::atomic::Ordering::SeqCst) {continue;}
        let executable=std::env::current_exe()?.with_file_name("pcscope-platform-native.exe");
        if Instant::now()>=modules_due {
            let result=collect_owned(&state,&executable,&["runtime-modules".into()],Some(serde_json::to_vec(&json!({"module_process_names":names}))?),Duration::from_secs(20),"runtime");
            let valid={let store=state.store.lock().unwrap();store.runtime_watch_settings()?["revision"]==config["revision"]&&store.item_settings()?["revision"]==item_revision};
            if valid {match result {
                Ok(result)=>{let mut store=state.store.lock().unwrap();for message in &result.messages{store.ingest(message)?;}store.observe_collection_success(&result.messages)?;
                    if manager.observe(&result.messages,&state.data_dir){store.save_runtime_watch_state(&serde_json::to_value(&manager.saved)?)?;}
                    module_failures=0;
                },
                Err(error)=>{module_failures=module_failures.saturating_add(1);persist_collector_failure(&state,SOURCES[0],&error);}
            }}
            modules_due=Instant::now()+Duration::from_secs((60u64.saturating_mul(1u64<<module_failures.min(4))).min(900));
        }
        if !file_enabled {continue;}
        for directory in manager.directories(&names,&state.data_dir) {
            if manager.watchers.contains_key(&directory)||manager.failed_dirs.get(&directory).is_some_and(|retry|*retry>Instant::now()){continue;}
            match notify::Watcher::open(Path::new(&directory)) {
                Ok(watcher)=>{manager.watchers.insert(directory.clone(),watcher);manager.failed_dirs.remove(&directory);
                    let keys:Vec<_>=manager.saved.files.iter().filter(|(_,f)|Path::new(&f.path).parent().is_some_and(|p|p.to_string_lossy().to_lowercase()==directory)).map(|(k,f)|(k.clone(),f.last_integrity.as_deref().and_then(|s|chrono::DateTime::parse_from_rfc3339(s).ok()).is_none_or(|checked|chrono::Utc::now().signed_duration_since(checked).num_seconds()>=3600))).collect();for (key,integrity) in keys{manager.queue(&key,"notification_started; earlier interval unknown",integrity,true);}},
                Err(error)=>{persist_collector_failure(&state,SOURCES[2],&anyhow::anyhow!("directory notification unavailable; periodic checking remains: {directory}: {error}"));manager.failed_dirs.insert(directory,Instant::now()+Duration::from_secs(60));manager.partial=true;}
            }
        }
        for gap in manager.notification_queue(){persist_collector_failure(&state,SOURCES[2],&anyhow::anyhow!(gap));}
        if Instant::now()>=poll_due||Instant::now()>=integrity_due {
            let integrity=Instant::now()>=integrity_due;
            let keys:Vec<_>=manager.saved.files.keys().filter(|k|manager.eligible(k,&names,&state.data_dir)).cloned().collect();
            for key in keys{manager.queue(&key,if integrity{"periodic_integrity"}else{"periodic_metadata"},integrity,true);}
            poll_due=Instant::now()+Duration::from_secs(300);if integrity{integrity_due=Instant::now()+Duration::from_secs(3600);}
        }
        let dispatch=Instant::now();
        for _ in 0..4 {
            if dispatch.elapsed()>=Duration::from_secs(2)||state.stopping.load(std::sync::atomic::Ordering::SeqCst){break;}
            let Some(key)=manager.due(&names,&state.data_dir) else{break};
            let pending=manager.pending.remove(&key).unwrap();let moved=manager.move_candidates.contains_key(&key);let file=manager.saved.files.get(&key).or_else(||manager.move_candidates.get(&key)).context("queued file missing")?.clone();
            let result=collect_owned(&state,&executable,&["module-files".into()],Some(serde_json::to_vec(&json!({"files":[file.path],"integrity":pending.integrity,"internal_data_dir":state.data_dir}))?),Duration::from_secs(5),"runtime");
            let valid={let store=state.store.lock().unwrap();store.runtime_watch_settings()?["revision"]==config["revision"]&&store.item_settings()?["revision"]==item_revision};
            if !valid {break;}
            let mut completed=false;
            match result {
                Ok(result)=>for message in result.messages {
                    let Message::Snapshot{source,scope,items,observed_at,..}=message.message else{continue};if source!=SOURCES[2]{continue;}
                    for item in items {
                        let mut after=item.data;let stable=after["status"]=="stable"||after["status"]=="missing";
                        if moved {
                            let previous=file.last_valid.clone().unwrap_or(Value::Null);
                            if !stable||after["exists"]!=true||previous["file_id"].is_null()||previous["volume_identity"]!=after["volume_identity"]||previous["file_id"]!=after["file_id"] {
                                manager.move_candidates.remove(&key);completed=true;manager.partial=true;
                                persist_collector_failure(&state,SOURCES[2],&anyhow::anyhow!("rename candidate identity could not be confirmed; not bound to loaded module: {}",file.path));continue;
                            }
                            if manager.saved.files.len()>=FILE_LIMIT {manager.move_candidates.remove(&key);completed=true;manager.partial=true;continue;}
                            manager.saved.files.insert(key.clone(),file.clone());manager.move_candidates.remove(&key);manager.saved.revision+=1;
                        }
                        let needs_integrity=stable&&after["exists"]==true&&!pending.integrity&&file.last_valid.as_ref().is_none_or(|before|["exists","file_id","volume_identity","size","last_write_utc"].iter().any(|k|before[*k]!=after[*k]));
                        if needs_integrity {after["hash_status"]=json!("pending");after["signature_status"]=json!("pending");}
                        let mut changes=if stable{differences(file.last_valid.as_ref(),&after)}else{vec!["check_unavailable".into()]};if moved{changes.push("file_moved".into());}
                        let record=json!({"path":file.path,"reason":pending.reason,"changes":changes,"check":after,"before":file.last_valid,"module_bindings":file.bindings,"auto_watch_revision":manager.saved.revision,"relationship_basis":"successful module observation; disk file at check time; not memory image identity"});
                        let mut store=state.store.lock().unwrap();
                        {
                            // Keep every actual check as immutable evidence. Compact unchanged-check
                            // summaries need a retention-safe reference contract before being enabled.
                            // Path reuse must not reuse a source binding for a different physical file.
                            let item_id=if after["status"]=="stable"&&after["exists"]==true{format!("{key}|{}|{}",after["volume_identity"].as_str().unwrap_or("unknown"),after["file_id"].as_str().unwrap_or("unknown"))}else{format!("{key}|{}",after["status"].as_str().unwrap_or("unknown"))};
                            store.ingest(&Envelope{protocol_version:1,request_id:format!("module-file/{key}/{observed_at}"),message:Message::Snapshot{source:SOURCES[2].into(),scope:scope.clone(),run_id:observed_at.clone(),observed_at:observed_at.clone(),complete:true,items:vec![Item{id:item_id,data:record}]}})?;
                        }
                        let saved=manager.saved.files.get_mut(&key).unwrap();saved.last_attempt=Some(after.clone());
                        if stable {
                            if saved.first_baseline.is_none(){saved.first_baseline=Some(after.clone());}
                            let mut merged=saved.last_valid.clone().unwrap_or(json!({}));
                            if pending.integrity||needs_integrity||after["exists"]==false {
                                for k in ["sha256","hash_checked_at","signature_checked_at","signature_code","signature_kind","signature_file_size","signature_file_last_write_utc","signature_policy","signature_error","publisher","signer_subject","signer_thumbprint","module_origin","catalog_path","pe_status","pe_machine","pe_format","pe_sections","pe_basis","pe_error"] {merged.as_object_mut().unwrap().remove(k);}
                                saved.last_integrity=None;
                            }
                            for (k,v) in after.as_object().context("native file result not structured")?{if v!="not_requested"{merged[k]=v.clone();}}
                            saved.last_valid=Some(merged);if pending.integrity{saved.last_integrity=Some(observed_at.clone());}
                            saved.failures=0;saved.retry_at=None;
                            manager.failures.remove(&key);completed=true;
                        }
                        store.save_runtime_watch_state(&serde_json::to_value(&manager.saved)?)?;
                        drop(store);if needs_integrity{manager.queue(&key,"metadata_changed; integrity_pending",true,true);}
                    }
                },
                Err(error)=>{
                    persist_collector_failure(&state,SOURCES[2],&error);
                    let time=chrono::Utc::now().to_rfc3339();let check=json!({"path":file.path,"status":"unavailable","error":error.to_string(),"checked_at":time,"signature_status":"unavailable","hash_status":"unavailable"});
                    if let Some(saved)=manager.saved.files.get_mut(&key){saved.last_attempt=Some(check.clone());}
                    state.store.lock().unwrap().ingest(&Envelope{protocol_version:1,request_id:format!("module-file-failure/{key}/{time}"),message:Message::Snapshot{source:SOURCES[2].into(),scope:format!("runtime-file/{}/{key}",state.endpoint.instance_id),run_id:time.clone(),observed_at:time,complete:true,items:vec![Item{id:key.clone(),data:json!({"path":file.path,"reason":pending.reason,"changes":["check_unavailable"],"check":check,"before":file.last_valid,"module_bindings":file.bindings,"auto_watch_revision":manager.saved.revision})}]}})?;
                }
            }
            if !completed {
                let failures=manager.saved.files.get(&key).map(|f|f.failures).unwrap_or(*manager.failures.get(&key).unwrap_or(&0)).saturating_add(1);
                manager.failures.insert(key.clone(),failures);let delay=(60u64.saturating_mul(1u64<<failures.saturating_sub(1).min(4))).min(900);
                if let Some(saved)=manager.saved.files.get_mut(&key){saved.failures=failures;saved.retry_at=Some((chrono::Utc::now()+chrono::Duration::seconds(delay as i64)).to_rfc3339());state.store.lock().unwrap().save_runtime_watch_state(&serde_json::to_value(&manager.saved)?)?;}
                manager.pending.insert(key,Pending{first:Instant::now(),last:Instant::now(),reason:"retry_previous_check_failed".into(),integrity:pending.integrity,not_before:Instant::now()+Duration::from_secs(delay)});
            }
        }
        if Instant::now()>=summary_due {summary(&state,&manager,true,file_enabled,"fixed scheduler observation; missing intervals are not reconstructed")?;summary_due=Instant::now()+Duration::from_secs(60);}
    }
    manager.watchers.clear();
    state.store.lock().unwrap().save_runtime_watch_state(&serde_json::to_value(&manager.saved)?)?;
    Ok(())
}
use anyhow::Context;
pub fn run(state:Arc<State>) {if let Err(error)=work(state.clone()){persist_collector_failure(&state,SOURCES[2],&error);eprintln!("runtime file monitor stopped: {error}");}}

#[cfg(test)]mod tests {
    use super::*;
    fn module(pid:u32,path:&str)->Envelope {Envelope{protocol_version:1,request_id:format!("fixture-{pid}"),message:Message::Snapshot{source:SOURCES[0].into(),scope:"fixture".into(),run_id:pid.to_string(),observed_at:"2026-10-12T00:00:00Z".into(),complete:true,items:vec![Item{id:path.into(),data:json!({"pid":pid,"process_start":"2026-10-12T00:00:00Z","process_name":"sihost","module_path":path})}]}}}
    #[test]fn automatic_scope_deduplicates_files_keeps_instances_and_excludes_internal_data()->Result<()> {
        let mut manager=Manager::new(json!({}))?;let own=Path::new(r"C:\PCScope\data");
        manager.observe(&[module(1,r"C:\Modules\one.dll"),module(2,r"c:\modules\ONE.dll"),module(3,r"C:\PCScope\data\private.dll")],own);
        assert_eq!(manager.saved.files.len(),1);assert_eq!(manager.saved.files.values().next().unwrap().bindings.len(),2);assert_eq!(manager.pending.len(),1);assert!(manager.partial);
        let restored=Manager::new(serde_json::to_value(&manager.saved)?)?;assert_eq!(restored.saved.files.len(),1);Ok(())
    }
    #[test]fn metadata_checks_do_not_erase_signatures_and_changes_do_not_mean_corruption() {
        let before=json!({"file_id":"one","sha256":"abc","signature_status":"valid_offline"});
        assert!(differences(Some(&before),&json!({"file_id":"one","signature_status":"not_requested"})).is_empty());
        assert_eq!(differences(Some(&before),&json!({"file_id":"two","sha256":"def"})),vec!["file_id","sha256"]);
    }
    #[test]fn restart_keeps_file_backoff_and_repeated_notifications_remain_one_pending_item()->Result<()> {
        let own=Path::new(r"C:\PCScope\data");let mut manager=Manager::new(json!({}))?;manager.observe(&[module(1,r"C:\Modules\one.dll")],own);
        let key=path_key(r"C:\Modules\one.dll");manager.saved.files.get_mut(&key).unwrap().retry_at=Some((chrono::Utc::now()+chrono::Duration::seconds(60)).to_rfc3339());
        let mut restored=Manager::new(serde_json::to_value(&manager.saved)?)?;
        for _ in 0..1000 {restored.queue(&key,"native_notification/action:3",true,true);}
        assert_eq!(restored.pending.len(),1);assert!(restored.due(&BTreeSet::from(["sihost".into()]),own).is_none());
        assert!(Manager::new(json!({"format_version":99})).is_err());Ok(())
    }
}
