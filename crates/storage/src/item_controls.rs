use crate::{Store,collection};
use anyhow::{Result,bail};
use pcscope_contracts::{Envelope,Message};
use rusqlite::OptionalExtension;
use serde_json::{json,Value};
use std::{collections::BTreeSet,sync::LazyLock};
static CONTROLS:LazyLock<Vec<Value>>=LazyLock::new(||serde_json::from_str(include_str!("../../../apps/desktop/ui/collection-controls.json")).expect("valid item controls"));
pub fn classify(source:&str,id:&str,kind:&str,data:&Value)->String {
 let mut rows:Vec<_>=collection::definitions().iter().collect();
 rows.sort_by_key(|r|(r["prefix"].as_bool().unwrap_or(false),r["contains"].is_null(),r["priority"].as_u64().unwrap_or(0)));
 for r in rows {
  let s=r["source"].as_str().unwrap();if !(if r["prefix"]==true{source.starts_with(s)}else{source==s}){continue;}
  if let Some(parts)=r["contains"].as_array(){if !parts.iter().any(|p|id.to_lowercase().contains(p.as_str().unwrap())){continue;}}
  if let Some(rule)=r.get("event_rule") {
   if rule["status"]==true&&!(kind.starts_with("coverage.")&&kind!="coverage.security_log_cleared"){continue;}
   if rule["provider"].is_string()&&rule["provider"]!=data["provider"]{continue;}
   if let Some(ids)=rule["event_ids"].as_array(){if !ids.contains(&data["event_id"]){continue;}}
  }
  return r["id"].as_str().unwrap().into();
 }source.into()
}

#[cfg(test)]mod tests{
 use super::*;
 fn snapshot(run:&str,items:Value)->Envelope{serde_json::from_value(json!({"protocol_version":1,"request_id":run,"type":"snapshot","source":"pcscope.shell.registrations","scope":"test","run_id":run,"observed_at":format!("2026-09-11T00:00:0{run}Z"),"complete":true,"items":items})).unwrap()}
 #[test]fn disabled_subitem_preserves_state_and_last_seen_and_reenable_compares(){
  let mut s=Store::open(":memory:").unwrap();let preview=r"software\microsoft\windows\currentversion\previewhandlers";let overlay=r"software\microsoft\windows\currentversion\explorer\shelliconoverlayidentifiers";
  assert!(s.item_controls().unwrap()["items"].as_array().unwrap().iter().all(|i|i["enabled"]==true));
  s.ingest(&snapshot("1",json!([{"id":preview,"data":{"version":1}},{"id":overlay,"data":{"version":1}}]))).unwrap();
  s.set_item_enabled("pcscope-powershell-adapter.exe","pcscope.shell.registrations.preview",false,"2026-09-11T00:00:02Z").unwrap();
  let revision=s.item_settings().unwrap()["revision"].clone();s.set_item_enabled("pcscope-powershell-adapter.exe","pcscope.shell.registrations.preview",false,"2026-09-11T00:00:02Z").unwrap();assert_eq!(s.item_settings().unwrap()["revision"],revision);
  s.ingest(&snapshot("3",json!([{"id":overlay,"data":{"version":2}},{"id":preview,"data":{"version":999}}]))).unwrap();
  let raw:String=s.conn.query_row("SELECT data FROM objects WHERE id=?",[preview],|r|r.get(0)).unwrap();assert_eq!(serde_json::from_str::<Value>(&raw).unwrap()["version"],1);
  let seen:String=s.conn.query_row("SELECT seen_at FROM object_times WHERE id=?",[preview],|r|r.get(0)).unwrap();assert_eq!(seen,"2026-09-11T00:00:01Z");
  assert!(!s.timeline(None,100).unwrap().iter().any(|r|r["kind"]=="object.no_longer_observed"));
  s.set_item_enabled("pcscope-powershell-adapter.exe","pcscope.shell.registrations.preview",true,"2026-09-11T00:00:04Z").unwrap();
  s.ingest(&snapshot("5",json!([{"id":preview,"data":{"version":3}},{"id":overlay,"data":{"version":2}}]))).unwrap();
  let rows=s.timeline(None,100).unwrap();let changed=rows.iter().find(|r|r["object_id"]==preview&&r["kind"]=="object.changed").unwrap();assert_eq!(changed["data"]["before"]["version"],1);assert_eq!(changed["data"]["before_observed_at"],"2026-09-11T00:00:01Z");
  assert!(s.set_item_enabled("system","pcscope.shell.registrations.preview",false,"now").is_err());
 }
 #[test]fn disabled_events_advance_checkpoint_without_history_and_classifier_matches_sql(){
  let mut s=Store::open(":memory:").unwrap();s.set_item_enabled("eventlog","pcscope.eventlog.security.crypto-keys",false,"2026-09-11T00:00:00Z").unwrap();
  let e:Envelope=serde_json::from_value(json!({"protocol_version":1,"request_id":"batch","type":"event_batch","source":"pcscope.eventlog.security","epoch":"test","batch_seq":1,"checkpoint":"next","events":[{"id":"1","occurred_at":"2026-09-11T00:00:01Z","kind":"windows.event","object_id":"5058","data":{"provider":"Microsoft-Windows-Security-Auditing","event_id":5058}},{"id":"2","occurred_at":"2026-09-11T00:00:01Z","kind":"windows.event","object_id":"4688","data":{"provider":"Microsoft-Windows-Security-Auditing","event_id":4688}}]})).unwrap();
  assert_eq!(s.ingest(&e).unwrap(),1);assert_eq!(s.checkpoint("pcscope.eventlog.security","test").unwrap(),Some("next".into()));assert!(!s.timeline(None,100).unwrap().iter().any(|r|r["data"]["event_id"]==5058));
  for (source,id,kind,data) in [("pcscope.eventlog.security","5058","windows.event",json!({"provider":"Microsoft-Windows-Security-Auditing","event_id":5058})),("pcscope.eventlog.security","","coverage.gap",json!({})),("pcscope.shell.registrations",r"software\classes\clsid\test","",json!({}))]{let sql=format!("SELECT {}",collection::event_item("?1","?2","?3","?4"));let actual:String=s.conn.query_row(&sql,rusqlite::params![source,id,kind,data.to_string()],|r|r.get(0)).unwrap();assert_eq!(classify(source,id,kind,&data),actual);}
 }
 #[test]fn item_settings_survive_reopen(){let path=std::env::temp_dir().join(format!("pcscope-items-{}-{}.db",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));{
  let mut s=Store::open(&path).unwrap();s.set_item_enabled("system","pcscope.identity.logon-sessions",false,"2026-09-11T00:00:00Z").unwrap();}
  {let s=Store::open(&path).unwrap();assert!(s.disabled_items().unwrap().contains("pcscope.identity.logon-sessions"));assert!(s.collection_selection("system").unwrap()["disabled_sources"].as_array().unwrap().contains(&json!("pcscope.identity.logon-sessions")));}std::fs::remove_file(path).unwrap();}
}
impl Store {
 pub fn item_settings(&self)->Result<Value>{Ok(self.conn.query_row("SELECT value FROM settings WHERE key='item_settings'",[],|r|r.get::<_,String>(0)).optional()?.map(|s|serde_json::from_str(&s)).transpose()?.unwrap_or(json!({"revision":0,"disabled":[]})))}
 pub fn disabled_items(&self)->Result<BTreeSet<String>>{Ok(serde_json::from_value(self.item_settings()?["disabled"].clone())?)}
 pub fn item_controls(&self)->Result<Value>{let settings=self.item_settings()?;let disabled=self.disabled_items()?;Ok(json!({"revision":settings["revision"],"items":CONTROLS.iter().map(|r|{let mut r=r.clone();r["enabled"]=json!(!disabled.contains(r["id"].as_str().unwrap()));r}).collect::<Vec<_>>()}))}
 pub fn set_item_enabled(&mut self,plugin:&str,id:&str,enabled:bool,time:&str)->Result<Value>{
  if !CONTROLS.iter().any(|r|r["plugin"]==plugin&&r["id"]==id){bail!("未知插件采集项");}
  let mut disabled=self.disabled_items()?;let changed=if enabled{disabled.remove(id)}else{disabled.insert(id.into())};
  if changed {let revision=self.item_settings()?["revision"].as_u64().unwrap_or(0)+1;let value=json!({"revision":revision,"disabled":disabled});
   self.conn.execute("INSERT INTO settings(key,value) VALUES('item_settings',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[value.to_string()])?;
   self.internal_event("collector.item_configured",time,json!({"plugin":plugin,"item":id,"enabled":enabled,"revision":revision}))?;
   // The coverage rows have to hear about this now rather than at the next collection: while an
   // item is off, no collection happens for it, so nothing else would ever record that the interval
   // it was off is missing from the history.
   let sources:Vec<String>={
    let mut q=self.conn.prepare("SELECT DISTINCT source FROM coverage_scopes")?;
    let rows=q.query_map([],|r|r.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
    rows.into_iter().filter(|source|crate::item_controls::classify(source,"","",&serde_json::Value::Null)==id).collect()
   };
   crate::coverage::set_configured(&self.conn,&sources,enabled,time)?;
  }self.item_controls()
 }
 pub fn collection_selection(&self,plugin:&str)->Result<Value>{let settings=self.item_settings()?;let disabled=self.disabled_items()?;let rows:Vec<_>=CONTROLS.iter().filter(|r|r["plugin"]==plugin).collect();let mut sources=BTreeSet::new();
  for row in &rows {let source=row["source"].as_str().unwrap();if rows.iter().filter(|r|r["source"]==source).all(|r|disabled.contains(r["id"].as_str().unwrap())){sources.insert(source);}}
  Ok(json!({"revision":settings["revision"],"disabled_items":rows.iter().filter_map(|r|r["id"].as_str()).filter(|id|disabled.contains(*id)).collect::<Vec<_>>(),"disabled_sources":sources,"all_disabled":!rows.is_empty()&&rows.iter().all(|r|disabled.contains(r["id"].as_str().unwrap())),"event_rules":if plugin=="eventlog"{rows.iter().map(|r|json!({"source":r["source"],"id":r["id"],"rule":r["event_rule"],"enabled":!disabled.contains(r["id"].as_str().unwrap())})).collect::<Vec<_>>()}else{vec![]}}))
 }
 pub(crate) fn select_envelope(&self,envelope:&Envelope)->Result<Option<Envelope>>{
  let disabled=self.disabled_items()?;if disabled.is_empty(){return Ok(Some(envelope.clone()));}
  let mut selected=envelope.clone();
  match &mut selected.message {
   Message::Snapshot{source,items,..}=>{
    let rows:Vec<_>=CONTROLS.iter().filter(|r|r["source"]==*source).collect();
    if !rows.is_empty()&&rows.iter().all(|r|disabled.contains(r["id"].as_str().unwrap())){return Ok(None);}
    items.retain(|i|!disabled.contains(&classify(source,&i.id,"",&i.data)));
   },
   Message::EventBatch{source,events,..}=>events.retain(|e|!disabled.contains(&classify(source,&e.object_id,&e.kind,&e.data))),
   Message::Gap{source,..}=>{if disabled.contains(&classify(source,"","coverage.gap",&json!({}))){return Ok(None);}}
  }Ok(Some(selected))
 }
}
