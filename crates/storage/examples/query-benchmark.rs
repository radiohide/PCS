use anyhow::{ensure,Result};
use rusqlite::{Connection,OpenFlags};
use serde_json::{json,Value};
use std::time::{Duration,Instant};
#[allow(dead_code)]
#[path="../src/collection.rs"] mod collection;

fn legacy_field(item:&str,field:&str,fallback:&str)->String {
    let quote=|s:&str|format!("'{}'",s.replace('\'',"''"));
    let mut sql=format!("CASE {item} ");
    for r in collection::definitions(){sql.push_str(&format!("WHEN {} THEN {} ",quote(r["id"].as_str().unwrap()),quote(r[field].as_str().unwrap())));}
    sql.push_str(&format!("ELSE {fallback} END"));sql
}
fn main()->Result<()> {
    let args:Vec<_>=std::env::args().collect();ensure!(args.len()==3,"query-benchmark <source-db> <new-copy-db>");
    if args[1]=="--existing-copy" {
        let start=Instant::now();drop(pcscope_storage::Store::open(&args[2])?);println!("Catalog migration: {} ms",start.elapsed().as_millis());
        for query in [json!({}),json!({"source":"pcscope.eventlog.security","item":"pcscope.eventlog.security.processes","action":"execution.process_started"})] {
            let store=pcscope_storage::Store::open_reader(&args[2],Duration::from_secs(6))?;let start=Instant::now();let result=store.search_timeline(&query,None,100)?;
            println!("Query {query}: {} ms; rows={}; first item={}",start.elapsed().as_millis(),result["items"].as_array().unwrap().len(),result["items"][0]["item_name"]);
        }
        return Ok(());
    }
    ensure!(!std::path::Path::new(&args[2]).exists(),"benchmark copy already exists");
    let source=Connection::open_with_flags(&args[1],OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    source.execute_batch("BEGIN;")?;
    let mut copy=Connection::open(&args[2])?;let started=Instant::now();
    rusqlite::backup::Backup::new(&source,&mut copy)?.run_to_completion(512,Duration::from_millis(1),None)?;
    source.execute_batch("ROLLBACK;")?;
    println!("Consistent copy: {} ms; records={}",started.elapsed().as_millis(),copy.query_row("SELECT count(*) FROM records",[],|r|r.get::<_,i64>(0))?);
    let deadline=Instant::now()+Duration::from_secs(8);copy.progress_handler(1000,Some(move||Instant::now()>deadline));
    let sql=format!("WITH classified AS (SELECT records.*,{} AS item FROM records),history AS (SELECT classified.*,{} AS item_name,{} AS category FROM classified) SELECT DISTINCT item,item_name,category FROM history ORDER BY category,item_name",collection::item("source","object_id"),legacy_field("item","name","source"),legacy_field("item","category","'other'"));
    let started=Instant::now();let legacy=(||->rusqlite::Result<usize>{let mut q=copy.prepare(&sql)?;let mut rows=q.query([])?;let mut count=0;while rows.next()?.is_some(){count+=1;}Ok(count)})();
    println!("Legacy facets: {} ms, {:?}",started.elapsed().as_millis(),legacy);drop(copy);
    let started=Instant::now();drop(pcscope_storage::Store::open(&args[2])?);println!("Index migration: {} ms",started.elapsed().as_millis());
    for (name,query) in [("default",json!({})),("item",json!({"item":"pcscope.startup.task-details"})),("kind",json!({"kind":"object.changed"})),("keyword",json!({"search":"MapsUpdateTask"})),("absent keyword",json!({"search":"pcscope-no-match-7b6c5d4e"}))] {
        let store=pcscope_storage::Store::open_reader(&args[2],Duration::from_secs(6))?;let started=Instant::now();
        let result=store.search_timeline(&query,None,100);println!("Timeline {name}: {} ms; {}",started.elapsed().as_millis(),match result{Ok(v)=>format!("rows={} facets={}",v["items"].as_array().unwrap().len(),v["sources"].as_array().unwrap().len()),Err(e)=>e.to_string()});
    }
    for (name,query) in [("default",json!({"sort":"item_asc"})),("keyword",json!({"search":"MapsUpdateTask"}))] {
        let store=pcscope_storage::Store::open_reader(&args[2],Duration::from_secs(6))?;let started=Instant::now();let result:Result<Value>=store.search_objects(&query,0,100);
        println!("Objects {name}: {} ms; {}",started.elapsed().as_millis(),match result{Ok(v)=>format!("total={}",v["total"]),Err(e)=>e.to_string()});
    }
    Ok(())
}
