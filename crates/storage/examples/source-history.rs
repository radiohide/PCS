use anyhow::{Context,Result};
use rusqlite::{Connection,OpenFlags};
use serde_json::{json,Value};
use std::{collections::BTreeMap,time::{Duration,Instant}};
fn main()->Result<()> {
    let path=std::env::args().nth(1).context("source-history <database> <source>")?;
    let source=std::env::args().nth(2).context("source required")?;
    let conn=Connection::open_with_flags(path,OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let deadline=Instant::now()+Duration::from_secs(8);
    conn.progress_handler(1000,Some(move||Instant::now()>=deadline));
    let mut q=conn.prepare("SELECT seq,kind,occurred_at,data FROM records WHERE source=? ORDER BY seq DESC")?;
    let mut rows=q.query([source])?;
    let mut groups:BTreeMap<String,Value>=BTreeMap::new();let mut total=0;
    let mut identities:BTreeMap<String,usize>=BTreeMap::new();let mut levels:BTreeMap<String,usize>=BTreeMap::new();let mut starts=Vec::new();
    while let Some(row)=rows.next()? {
        total+=1;
        let seq:i64=row.get(0)?;let kind:String=row.get(1)?;let time:String=row.get(2)?;
        let mut data:Value=serde_json::from_str(&row.get::<_,String>(3)?)?;
        pcscope_contracts::sensitive::present(&mut data,false);
        *levels.entry(data["level"].to_string()).or_default()+=1;
        let identity=format!("{} / {} / {} / {}",data["record_id"],time,data["event_id"],data["xml_sha256"]);
        let occurrences=identities.entry(identity).or_default();*occurrences+=1;let unique=*occurrences==1;
        if unique&&data["event_id"]==4115&&starts.len()<30 {starts.push(json!({"time":time,"record_id":data["record_id"],"fields":data["fields"]}));}
        let key=format!("{} / {} / {}",data["provider"].as_str().unwrap_or(""),data["event_id"],kind);
        let group=groups.entry(key).or_insert_with(||json!({"count":0,"unique_records":0,"first":time,"last":time,"samples":[]}));
        group["count"]=json!(group["count"].as_u64().unwrap()+1);
        if unique{group["unique_records"]=json!(group["unique_records"].as_u64().unwrap()+1);}
        if time.as_str()<group["first"].as_str().unwrap(){group["first"]=json!(time);}
        if time.as_str()>group["last"].as_str().unwrap(){group["last"]=json!(time);}
        if group["samples"].as_array().unwrap().len()<2 {
            let message=data["message"].as_str().or(data["reason"].as_str()).unwrap_or("").chars().take(1800).collect::<String>();
            group["samples"].as_array_mut().unwrap().push(json!({"seq":seq,"time":time,"level":data["level"],"message":message,"fields":data["fields"],"correlation":data["system"]["Correlation"]}));
        }
    }
    let repeated:Vec<_>=identities.iter().filter(|(_,n)|**n>1).take(5).map(|(id,n)|json!({"identity":id,"copies":n})).collect();
    println!("{}",serde_json::to_string_pretty(&json!({"total":total,"groups":groups,"levels":levels,"unique_records":identities.len(),"duplicate_examples":repeated,"recent_service_starts":starts}))?);
    Ok(())
}
