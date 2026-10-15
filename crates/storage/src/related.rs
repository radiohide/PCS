use crate::Store;
use anyhow::Result;
use rusqlite::params;
use serde_json::{json,Value};
fn pid(v:&Value)->Option<i64>{let raw=match v{Value::String(s)=>s.clone(),Value::Number(n)=>n.to_string(),_=>return None};let raw=raw.trim();let n=if raw.starts_with("0x")||raw.starts_with("0X"){i64::from_str_radix(&raw[2..],16).ok()}else{raw.parse().ok()};n.filter(|n|*n>0)}
impl Store {
 pub fn event_related(&self,seq:i64)->Result<Value>{
  let (source,time,raw):(String,String,String)=self.conn.query_row("SELECT source,occurred_at,data FROM records WHERE seq=?",[seq],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
  let d:Value=serde_json::from_str(&raw)?;let f=&d["fields"];
  if !source.starts_with("pcscope.eventlog."){return Ok(json!({"items":[],"basis":"此记录没有事件关联标识"}));}
  let security=source=="pcscope.eventlog.security"&&d["provider"]=="Microsoft-Windows-Security-Auditing";
  let task=f["TaskName"].as_str().filter(|s|!s.is_empty()&&*s!="******");
  let client=if security{pid(&f["ClientProcessId"])}else{None};
  let created=f["ClientCreationTime"].as_str();
  let mut events=Vec::new();
  // PID alone is never used as a process identity. Return explicitly labelled candidates.
  if let (Some(client),Some(created))=(client,created){
   let hex=format!("0x{client:x}");let decimal=client.to_string();
   let mut stmt=self.conn.prepare("SELECT r.seq,r.source,r.kind,r.object_id,r.occurred_at,r.data FROM record_search s JOIN records r ON r.seq=s.seq WHERE s.action='execution.process_started' AND r.seq<>?1 AND julianday(r.occurred_at)<=julianday(?2) AND abs((julianday(r.occurred_at)-julianday(?3))*86400.0)<=2 AND ((r.source='pcscope.eventlog.security' AND json_extract(r.data,'$.provider')='Microsoft-Windows-Security-Auditing' AND json_extract(r.data,'$.event_id')=4688 AND (lower(CAST(json_extract(r.data,'$.fields.NewProcessId') AS TEXT))=?4 OR CAST(json_extract(r.data,'$.fields.NewProcessId') AS TEXT)=?5)) OR (r.source='pcscope.eventlog.microsoft-windows-sysmon/operational' AND json_extract(r.data,'$.provider')='Microsoft-Windows-Sysmon' AND json_extract(r.data,'$.event_id')=1 AND CAST(json_extract(r.data,'$.fields.ProcessId') AS TEXT)=?5)) ORDER BY r.seq DESC LIMIT 21")?;
   let rows=stmt.query_map(params![seq,time,created,hex,decimal],related_row)?;for row in rows{let mut row=row?;row["relation"]=json!("同 PID、建立时间附近（±2 秒）的进程启动记录");events.push(row);}
  }
  if let Some(task)=task {
   let mut stmt=self.conn.prepare("SELECT seq,source,kind,object_id,occurred_at,data FROM records WHERE seq<>?1 AND source IN ('pcscope.eventlog.security','pcscope.eventlog.microsoft-windows-taskscheduler/operational') AND json_extract(data,'$.fields.TaskName')=?2 COLLATE NOCASE AND abs((julianday(occurred_at)-julianday(?3))*86400.0)<=300 ORDER BY seq DESC LIMIT 21")?;
   let rows=stmt.query_map(params![seq,task,time],related_row)?;for row in rows{let mut row=row?;row["relation"]=json!("同任务路径、前后 5 分钟的事件");events.push(row);}
  }
  let truncated=events.len()>20;events.truncate(20);
  self.attach_activity(&mut events)?;
  Ok(json!({"items":events,"truncated":truncated,"basis":if task.is_none()&&(client.is_none()||created.is_none()){"未记录可用的关联标识"}else{"按对象标识和时间匹配"}}))
 }
}
fn related_row(r:&rusqlite::Row<'_>)->rusqlite::Result<Value>{let raw:String=r.get(5)?;let data:Value=serde_json::from_str(&raw).map_err(|e|rusqlite::Error::FromSqlConversionFailure(5,rusqlite::types::Type::Text,Box::new(e)))?;Ok(json!({"seq":r.get::<_,i64>(0)?,"source":r.get::<_,String>(1)?,"kind":r.get::<_,String>(2)?,"object_id":r.get::<_,String>(3)?,"occurred_at":r.get::<_,String>(4)?,"data":data}))}
#[cfg(test)]mod tests{use super::*;#[test]fn process_id_requires_valid_nonzero_value(){assert_eq!(pid(&json!("0x1BB8")),Some(7096));assert_eq!(pid(&json!(7096)),Some(7096));for v in [json!(0),json!(-1),json!("******"),json!(null),json!("0x"),json!("7096 OR 1=1")]{assert_eq!(pid(&v),None);}}}
#[cfg(test)]mod integration_tests {
 use super::*;
 #[test]fn related_events_require_pid_time_and_provider_and_do_not_write(){
  let store=Store::open(":memory:").unwrap();
  let insert=|time:&str,data:Value|{store.conn.execute("INSERT INTO records(source,kind,object_id,occurred_at,data) VALUES('pcscope.eventlog.security','windows.event','test',?,?)",params![time,data.to_string()]).unwrap();store.conn.last_insert_rowid()};
  let valid=insert("2026-09-11T09:14:42.206Z",json!({"provider":"Microsoft-Windows-Security-Auditing","event_id":4688,"fields":{"NewProcessId":"0x1bb8","NewProcessName":"example.exe"}}));
  insert("2026-09-11T08:14:42Z",json!({"provider":"Microsoft-Windows-Security-Auditing","event_id":4688,"fields":{"NewProcessId":"0x1bb8"}}));
  insert("2026-09-11T09:14:42.206Z",json!({"provider":"other","event_id":4688,"fields":{"NewProcessId":"0x1bb8"}}));
  insert("2026-09-11T09:14:42.206Z",json!({"provider":"Microsoft-Windows-Security-Auditing","event_id":4688,"fields":{"NewProcessId":"0x620"}}));
  let seq=insert("2026-09-11T09:14:44Z",json!({"provider":"Microsoft-Windows-Security-Auditing","event_id":5058,"writer_process_id":1568,"fields":{"ClientProcessId":"7096","ClientCreationTime":"2026-09-11T09:14:42.206Z"}}));
  let result=store.event_related(seq).unwrap();assert_eq!(result["items"].as_array().unwrap().len(),1);assert_eq!(result["items"][0]["seq"],valid);
  let missing=insert("2026-09-11T09:14:44Z",json!({"provider":"Microsoft-Windows-Security-Auditing","event_id":5058,"fields":{"ClientProcessId":"7096"}}));
  assert!(store.event_related(missing).unwrap()["items"].as_array().unwrap().is_empty());
  let count:i64=store.conn.query_row("SELECT count(*) FROM records",[],|r|r.get(0)).unwrap();assert_eq!(count,6);
 }
}
