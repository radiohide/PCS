use anyhow::{Context,Result};
use rusqlite::{Connection,OpenFlags};
use serde_json::{json,Value};
use std::time::{Duration,Instant};
fn main()->Result<()> {
 let path=std::env::args().nth(1).context("event-audit <database>")?;
 let conn=Connection::open_with_flags(path,OpenFlags::SQLITE_OPEN_READ_ONLY)?;
 let deadline=Instant::now()+Duration::from_secs(45);conn.progress_handler(5000,Some(move||Instant::now()>=deadline));
 let tx=conn.unchecked_transaction()?;
 let mut q=tx.prepare("SELECT r.source,json_extract(r.data,'$.provider'),json_extract(r.data,'$.event_id'),s.item,count(*),max(r.seq) FROM records r JOIN record_search s ON r.seq=s.seq WHERE r.source LIKE 'pcscope.eventlog.%' AND r.kind NOT LIKE 'coverage.%' GROUP BY r.source,json_extract(r.data,'$.provider'),json_extract(r.data,'$.event_id'),s.item ORDER BY count(*) DESC")?;
 let rows=q.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,Option<String>>(1)?,r.get::<_,Option<i64>>(2)?,r.get::<_,String>(3)?,r.get::<_,i64>(4)?,r.get::<_,i64>(5)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
 let mut result=Vec::new();
 for (source,provider,id,item,count,seq) in rows {
  let raw:String=tx.query_row("SELECT data FROM records WHERE seq=?",[seq],|r|r.get(0))?;
  let mut d:Value=serde_json::from_str(&raw)?;pcscope_contracts::sensitive::present(&mut d,false);
  result.push(json!({"source":source,"provider":provider,"event_id":id,"item":item,"count":count,"sample_seq":seq,"message":d["message"].as_str().unwrap_or("").chars().take(2400).collect::<String>(),"fields":d["fields"],"level":d["level"]}));
 }
 println!("{}",serde_json::to_string_pretty(&result)?);Ok(())
}
