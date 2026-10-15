use anyhow::Result;
use rusqlite::{params,Connection};
use serde_json::{json,Value};
use crate::Store;
pub fn ensure(conn:&Connection)->Result<()> {
 conn.execute_batch("CREATE TABLE IF NOT EXISTS own_processes(pid INTEGER NOT NULL,created_ms INTEGER NOT NULL,seen_ms INTEGER NOT NULL,ended_ms INTEGER,image TEXT NOT NULL,role TEXT NOT NULL,plugin TEXT NOT NULL,run_id TEXT NOT NULL,PRIMARY KEY(pid,created_ms));
 CREATE INDEX IF NOT EXISTS own_processes_seen ON own_processes(seen_ms);
 CREATE TABLE IF NOT EXISTS activity_evidence(seq INTEGER PRIMARY KEY REFERENCES records(seq) ON DELETE CASCADE,pid INTEGER,created_ms INTEGER,at_ms INTEGER NOT NULL,image TEXT,parent_pid INTEGER,parent_image TEXT);
 CREATE INDEX IF NOT EXISTS activity_evidence_pid ON activity_evidence(pid,at_ms);
 CREATE INDEX IF NOT EXISTS activity_evidence_parent ON activity_evidence(parent_pid,at_ms);
 CREATE TABLE IF NOT EXISTS self_activity(seq INTEGER PRIMARY KEY REFERENCES records(seq) ON DELETE CASCADE,detail TEXT NOT NULL);")?;Ok(())
}

#[cfg(test)] mod tests {
 use super::*;
 fn event(s:&Store,source:&str,provider:&str,id:u32,time:&str,fields:Value,writer:u32)->i64 {
  s.conn.execute("INSERT INTO records(source,kind,object_id,occurred_at,data) VALUES(?,'windows.event','test',?,?)",params![source,time,json!({"provider":provider,"event_id":id,"fields":fields,"writer_process_id":writer}).to_string()]).unwrap();
  s.conn.last_insert_rowid()
 }
 const SOURCE:&str="pcscope.eventlog.security";
 const PROVIDER:&str="Microsoft-Windows-Security-Auditing";
 fn observe(s:&Store,seen:&str){s.observe_own_process(42,millis(&s.conn,"2026-09-11T00:00:00Z").unwrap().unwrap(),millis(&s.conn,seen).unwrap().unwrap(),None,r"C:\PCScope\collector.exe","collector","users","run-1").unwrap();}
 fn tagged(s:&Store,seq:i64)->bool{s.conn.query_row("SELECT EXISTS(SELECT 1 FROM self_activity WHERE seq=?)",[seq],|r|r.get(0)).unwrap()}
 #[test] fn identity_lifetime_provider_and_paging_preserve_all_records(){
  let s=Store::open(":memory:").unwrap();observe(&s,"2026-09-11T00:00:10Z");
  let good=event(&s,SOURCE,PROVIDER,4798,"2026-09-11T00:00:05Z",json!({"CallerProcessId":"0x2a","CallerProcessName":"C:\\PCScope\\collector.exe"}),999);
  let key=event(&s,SOURCE,PROVIDER,5058,"2026-09-11T00:00:06Z",json!({"ClientProcessId":42,"ClientCreationTime":"2026-09-11T00:00:00Z"}),999);
  let parent=event(&s,SOURCE,PROVIDER,4688,"2026-09-11T00:00:07Z",json!({"NewProcessId":43,"NewProcessName":"C:\\child.exe","ProcessId":42,"ParentProcessName":"C:\\PCScope\\collector.exe"}),999);
  let invalid=[
    event(&s,SOURCE,PROVIDER,5058,"2026-09-11T00:00:05Z",json!({}),42),
    event(&s,SOURCE,PROVIDER,4798,"2026-09-11T00:00:05Z",json!({"CallerProcessId":42,"CallerProcessName":"C:\\other\\collector.exe"}),0),
    event(&s,SOURCE,PROVIDER,5058,"2026-09-11T00:00:05Z",json!({"ClientProcessId":42,"ClientCreationTime":"2026-09-10T00:00:00Z"}),0),
    event(&s,SOURCE,"Other",4798,"2026-09-11T00:00:05Z",json!({"CallerProcessId":42,"CallerProcessName":"C:\\PCScope\\collector.exe"}),0),
    event(&s,SOURCE,PROVIDER,4798,"2026-09-11T00:00:15Z",json!({"CallerProcessId":42,"CallerProcessName":"C:\\PCScope\\collector.exe"}),0),
    event(&s,SOURCE,PROVIDER,4798,"2026-09-10T23:59:59Z",json!({"CallerProcessId":42,"CallerProcessName":"C:\\PCScope\\collector.exe"}),0),
  ];
  ingest(&s.conn,0).unwrap();for seq in [good,key,parent]{assert!(tagged(&s,seq));}for seq in invalid{assert!(!tagged(&s,seq));}
  let only=s.search_timeline(&json!({"self_activity":"only"}),None,2).unwrap();assert_eq!(only["items"][0]["seq"],parent);assert_eq!(only["items"][1]["seq"],key);assert_eq!(only["more"],true);
  let page=s.search_timeline(&json!({"self_activity":"only"}),Some(key),2).unwrap();assert_eq!(page["items"].as_array().unwrap().len(),1);assert_eq!(page["items"][0]["seq"],good);
  assert_eq!(s.search_timeline(&json!({"self_activity":"hide"}),None,100).unwrap()["items"].as_array().unwrap().len(),6);
  assert_eq!(s.timeline(None,100).unwrap().len(),9);
  assert!(s.search_timeline(&json!({"self_activity":"invalid"}),None,100).is_err());
  observe(&s,"2026-09-11T00:00:20Z");assert!(tagged(&s,invalid[4]));
  s.conn.execute("DELETE FROM records WHERE seq=?",[good]).unwrap();assert!(!tagged(&s,good));
  let evidence:i64=s.conn.query_row("SELECT count(*) FROM activity_evidence WHERE seq=?",[good],|r|r.get(0)).unwrap();assert_eq!(evidence,0);
 }
 #[test] fn late_native_observation_and_powershell_host_are_explicit(){
  let s=Store::open(":memory:").unwrap();
  let source="pcscope.eventlog.microsoft-windows-powershell/operational";
  let good=event(&s,source,"Microsoft-Windows-PowerShell",4104,"2026-09-11T00:00:05Z",json!({"ScriptBlockText":"Get-Process"}),42);
  let other=event(&s,source,"Other",4104,"2026-09-11T00:00:05Z",json!({}),42);
  let audit=event(&s,SOURCE,PROVIDER,4662,"2026-09-11T00:00:05Z",json!({}),42);
  ingest(&s.conn,0).unwrap();assert!(!tagged(&s,good));
  s.observe_own_process(42,millis(&s.conn,"2026-09-11T00:00:00Z").unwrap().unwrap(),millis(&s.conn,"2026-09-11T00:00:10Z").unwrap().unwrap(),None,r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe","collector","users","run-1").unwrap();
  assert!(tagged(&s,good));assert!(!tagged(&s,other));assert!(!tagged(&s,audit));
  let rows=s.timeline(None,100).unwrap();let row=rows.iter().find(|r|r["seq"]==good).unwrap();assert_eq!(row["activity"]["basis"],"verified_powershell_host");assert_eq!(row["data"]["fields"]["ScriptBlockText"],"Get-Process");
 }
}
fn pid(v:&Value)->Option<i64>{let s=match v{Value::String(s)=>s.clone(),Value::Number(n)=>n.to_string(),_=>return None};let n=if s.starts_with("0x")||s.starts_with("0X"){i64::from_str_radix(&s[2..],16).ok()}else{s.parse().ok()};n.filter(|n|*n>0)}
fn image(v:&Value)->Option<String>{v.as_str().filter(|s|!s.is_empty()&&*s!="******").map(|s|s.trim_start_matches("\\\\?\\").replace('/',"\\").to_lowercase())}
fn millis(conn:&Connection,v:&str)->Result<Option<i64>>{Ok(conn.query_row("SELECT CAST(round((julianday(?)-2440587.5)*86400000) AS INTEGER)",[v],|r|r.get(0))?)}
pub fn ingest(conn:&Connection,after:i64)->Result<()> {
 let mut stmt=conn.prepare("SELECT seq,source,occurred_at,data FROM records WHERE seq>? AND source LIKE 'pcscope.eventlog.%' AND kind NOT LIKE 'coverage.%'")?;
 let rows=stmt.query_map([after],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
 for(seq,source,time,raw)in rows{
  let d:Value=serde_json::from_str(&raw)?;let f=&d["fields"];let provider=d["provider"].as_str().unwrap_or("");let id=d["event_id"].as_u64().unwrap_or(0);
  let security=source=="pcscope.eventlog.security"&&provider=="Microsoft-Windows-Security-Auditing";
  let sysmon=source=="pcscope.eventlog.microsoft-windows-sysmon/operational"&&provider=="Microsoft-Windows-Sysmon";
  let wmi=source=="pcscope.eventlog.microsoft-windows-wmi-activity/operational"&&provider=="Microsoft-Windows-WMI-Activity";
  let ps=source=="pcscope.eventlog.microsoft-windows-powershell/operational"&&provider=="Microsoft-Windows-PowerShell";
  if !security&&!sysmon&&!wmi&&!ps{continue;}
  let creation=(security&&id==4688)||(sysmon&&id==1);
  let client=pid(&f["ClientProcessId"]);
  let host=ps&&matches!(id,4103|4104);
  let process=if host{pid(&d["writer_process_id"])}else if creation&&security{pid(&f["NewProcessId"])}else{client.or_else(||pid(&f["CallerProcessId"])).or_else(||pid(&f["SourceProcessId"])).or_else(||pid(&f["ProcessId"])).or_else(||pid(&f["ProcessID"]))};
  let path=if host{Some("@powershell-host".into())}else if creation&&security{image(&f["NewProcessName"])}else{image(&f["ClientProcessName"]).or_else(||image(&f["CallerProcessName"])).or_else(||image(&f["SourceImage"])).or_else(||image(&f["Image"])).or_else(||image(&f["ProcessName"])).or_else(||image(&f["Application"]))};
  let parent=if creation{if security{pid(&f["ProcessId"])}else{pid(&f["ParentProcessId"])}}else{None};
  let parent_path=if creation{image(&f["ParentProcessName"]).or_else(||image(&f["ParentImage"]))}else{None};
  let created=if client.is_some(){f["ClientCreationTime"].as_str().map(|v|millis(conn,v)).transpose()?.flatten()}else{None};
  let Some(at)=millis(conn,&time)? else{continue;};
  if process.is_none()&&parent.is_none(){continue;}
  conn.execute("INSERT OR REPLACE INTO activity_evidence VALUES(?,?,?,?,?,?,?)",params![seq,process,created,at,path,parent,parent_path])?;
 }
 reconcile(conn,&format!("e.seq>{after}"))?;Ok(())
}
fn reconcile(conn:&Connection,restriction:&str)->Result<()> {
 // Security/WMI writer PIDs are never callers. The narrowly scoped PowerShell
 // exception identifies only the verified host of 4103/4104, not arbitrary callers.
 // No basename-only or time-only attribution. Process must have
 // been verified alive during this time interval through a native process handle.
 conn.execute_batch(&format!("INSERT OR IGNORE INTO self_activity(seq,detail)
 SELECT e.seq,json_object('classification','self','role',CASE WHEN e.image='@powershell-host' THEN 'powershell-host' ELSE p.role END,'plugin',p.plugin,'run_id',p.run_id,'pid',p.pid,'created_ms',p.created_ms,'image',p.image,'basis',CASE WHEN e.image='@powershell-host' THEN 'verified_powershell_host' ELSE 'native_process_lifetime' END)
 FROM activity_evidence e JOIN own_processes p ON p.pid=e.pid AND e.at_ms>=p.created_ms AND e.at_ms<=COALESCE(p.ended_ms,p.seen_ms)
 WHERE ({restriction}) AND ((e.created_ms IS NOT NULL AND abs(e.created_ms-p.created_ms)<=1) OR (e.created_ms IS NULL AND e.image=p.image) OR (e.image='@powershell-host' AND (p.image LIKE '%\\powershell.exe' OR p.image LIKE '%\\pwsh.exe')));
 INSERT OR IGNORE INTO self_activity(seq,detail)
 SELECT e.seq,json_object('classification','self','role','child-launch','plugin',p.plugin,'run_id',p.run_id,'pid',p.pid,'created_ms',p.created_ms,'image',p.image,'basis','verified_parent_created_process')
 FROM activity_evidence e JOIN own_processes p ON p.pid=e.parent_pid AND e.parent_image=p.image AND e.at_ms>=p.created_ms AND e.at_ms<=COALESCE(p.ended_ms,p.seen_ms) WHERE ({restriction});"))?;Ok(())
}
impl Store {
 pub fn observe_own_process(&self,pid:u32,created_ms:i64,seen_ms:i64,ended_ms:Option<i64>,image_path:&str,role:&str,plugin:&str,run_id:&str)->Result<()> {
  let tx=self.conn.unchecked_transaction()?;
  let image_path=image(&json!(image_path)).unwrap_or_default();
  self.conn.execute("INSERT INTO own_processes VALUES(?,?,?,?,?,?,?,?) ON CONFLICT(pid,created_ms) DO UPDATE SET seen_ms=max(own_processes.seen_ms,excluded.seen_ms),ended_ms=COALESCE(excluded.ended_ms,own_processes.ended_ms)",params![pid,created_ms,seen_ms,ended_ms,image_path,role,plugin,run_id])?;
  reconcile(&self.conn,&format!("p.pid={pid}"))?;tx.commit()?;Ok(())
 }
 pub fn attach_activity(&self,rows:&mut [Value])->Result<()> {
  for row in rows{let Some(seq)=row["seq"].as_i64()else{continue;};let detail:Option<String>=self.conn.query_row("SELECT (SELECT detail FROM self_activity WHERE seq=?)",[seq],|r|r.get(0))?;if let Some(detail)=detail{row["activity"]=serde_json::from_str(&detail)?;}}
  Ok(())
 }
}
