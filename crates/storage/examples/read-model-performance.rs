//! Bounded, local model rehearsal. Production material is attached read-only; only model tables
//! are copied, avoiding a second copy of a large raw-history database. No collectors are started.
use anyhow::{ensure,Context,Result};
use pcscope_contracts::query::{ActionFilter,InstanceFilter,InstanceSort,QueryRequest};
use pcscope_storage::Store;
use rusqlite::{Connection,OpenFlags};
use serde_json::{json,Value};
use std::{path::{Path,PathBuf},time::{Duration,Instant}};

const HOT:&[&str]=&["object_instances","source_bindings","object_facts","fact_history","object_actions",
    "type_definitions","action_definitions","type_actions","source_map","coverage_scopes","projection_state",
    "display_objects","display_actions","display_work","display_identity_work","object_collection_scopes",
    "display_build","display_totals","display_evidence_totals"];
const RAW:&[&str]=&["records","snapshots","objects","object_times","scopes","evidence","evidence_refs","projection_queue","coverage_state","checkpoints"];
fn fixture(path:&Path)->Result<PathBuf>{
    let root=std::env::current_dir()?.join("target").canonicalize()?;
    let path=path.canonicalize()?;
    ensure!(path.starts_with(&root)&&path!=root,"fixture must be under this workspace's target directory");
    Ok(path)
}
fn origin(conn:&Connection,path:&Path)->Result<()> {
    let text=path.to_str().context("origin path must be Unicode")?;
    let text=if let Some(unc)=text.strip_prefix(r"\\?\UNC\"){format!(r"\\{unc}")}else{text.strip_prefix(r"\\?\").unwrap_or(text).to_owned()};
    let uri=format!("file:{}?mode=ro",text.replace('\\',"/").replace('%',"%25").replace('?',"%3F").replace('#',"%23"));
    conn.execute("ATTACH DATABASE ? AS origin",[uri]).context("attaching read-only production material")?;
    // SQLite enforces the origin URI's read-only mode even while the local rehearsal is writable.
    Ok(())
}
fn prepare(source:&Path,directory:&Path)->Result<()> {
    let source=source.canonicalize()?;let directory=fixture(directory)?;let dest=directory.join("history.db");
    ensure!(!dest.exists(),"refusing to overwrite a fixture");
    let check=Connection::open_with_flags(&source,OpenFlags::SQLITE_OPEN_READ_ONLY).context("opening read-only source")?;
    let version:i64=check.query_row("PRAGMA user_version",[],|r|r.get(0))?;
    ensure!(version==pcscope_storage::model::SCHEMA_VERSION,"source schema mismatch");drop(check);
    let store=Store::open(&dest).context("creating isolated fixture")?;let conn=store.reader();origin(conn,&source)?;
    let triggers={let mut q=conn.prepare("SELECT name FROM main.sqlite_schema WHERE type='trigger'")?;let rows=q.query_map([],|r|r.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;rows};
    conn.execute_batch("PRAGMA foreign_keys=OFF;BEGIN;")?;
    for name in triggers {conn.execute_batch(&format!("DROP TRIGGER \"{}\"",name.replace('"',"\"\"")))?;}
    let started=Instant::now();let mut counts=json!({});
    for table in HOT {
        conn.execute(&format!("DELETE FROM main.{table}"),[])?;
        let count=conn.execute(&format!("INSERT INTO main.{table} SELECT * FROM origin.{table}"),[])?;
        counts[*table]=json!(count);
    }
    conn.execute_batch("COMMIT;DETACH DATABASE origin;PRAGMA wal_checkpoint(TRUNCATE);")?;drop(store);
    std::fs::write(directory.join("performance-origin.json"),serde_json::to_vec_pretty(&json!({"source":source,"schema":version,"hot_rows":counts,"prepared_seconds":started.elapsed().as_secs_f64(),"raw_copied":false}))?)?;
    println!("{}",json!({"prepared":directory,"hot_rows":counts,"seconds":started.elapsed().as_secs_f64(),"production":"read_only"}));Ok(())
}
fn open(directory:&Path)->Result<Store>{
    let directory=fixture(directory)?;
    let manifest:Value=serde_json::from_slice(&std::fs::read(directory.join("performance-origin.json"))?)?;
    let source=PathBuf::from(manifest["source"].as_str().context("source path missing")?);
    let store=Store::open(directory.join("history.db"))?;let conn=store.reader();origin(conn,&source)?;
    conn.execute_batch("DROP INDEX IF EXISTS main.evidence_refs_observation_address;")?;
    for table in RAW {conn.execute_batch(&format!("CREATE TEMP VIEW {table} AS SELECT * FROM origin.{table};"))?;}
    Ok(store)
}
fn percentile(values:&[f64],p:f64)->f64{values[((values.len() as f64*p).ceil() as usize).saturating_sub(1).min(values.len()-1)]}
fn query(store:&Store,request:&QueryRequest)->Result<pcscope_contracts::query::QueryResponse>{
    let deadline=Instant::now()+Duration::from_secs(6);
    store.reader().progress_handler(1000,Some(move||Instant::now()>=deadline));
    store.query(request)
}
fn run(directory:&Path,samples:usize,anchor_index:bool)->Result<()> {
    ensure!((1..=1000).contains(&samples),"samples must be 1..1000");let store=open(directory)?;
    if !anchor_index{store.reader().execute_batch("DROP INDEX IF EXISTS object_actions_anchor;")?;}
    println!("{}",json!({"anchor_index":anchor_index,"fixture":directory,"raw":"read_only_attached","samples":samples}));
    measure(&store,samples)
}
fn measure(store:&Store,samples:usize)->Result<()> {
    ensure!((1..=1000).contains(&samples),"samples must be 1..1000");
    let count_mode=std::env::var("PCSCOPE_PERFORMANCE_LIST_COUNTS").unwrap_or_else(|_|"full".into());
    ensure!(matches!(count_mode.as_str(),"full"|"objects"),"list counts mode must be full or objects");
    println!("{}",json!({"list_counts":count_mode,"includes_ipc":false}));
    let first=query(store,&QueryRequest::InstanceList(InstanceFilter{limit:100,..Default::default()}))?;
    let detail=first.data.as_ref().and_then(Value::as_array).and_then(|rows|rows.first()).and_then(|row|row["instance_id"].as_i64()).context("fixture has no objects")?.to_string();
    let first_actions=query(store,&QueryRequest::ActionTimeline(ActionFilter{limit:200,..Default::default()}))?;
    let mut cases=vec![
        ("objects_first",QueryRequest::InstanceList(InstanceFilter{limit:100,..Default::default()})),
        ("objects_second",QueryRequest::InstanceList(InstanceFilter{limit:100,include_counts:false,cursor:first.page.as_ref().and_then(|p|p.next_cursor.clone()),..Default::default()})),
        ("timeline_first",QueryRequest::ActionTimeline(ActionFilter{limit:200,..Default::default()})),
        ("timeline_second",QueryRequest::ActionTimeline(ActionFilter{limit:200,cursor:first_actions.page.as_ref().and_then(|p|p.next_cursor.clone()),..Default::default()})),
        ("object_detail",QueryRequest::InstanceDetail{instance:detail}),
        ("progress",QueryRequest::ProjectionStatus),
        ("objects_network",QueryRequest::InstanceList(InstanceFilter{category:Some("network".into()),limit:100,..Default::default()})),
        ("objects_processes",QueryRequest::InstanceList(InstanceFilter{type_id:Some("runtime.process".into()),limit:100,..Default::default()})),
        ("objects_recent",QueryRequest::InstanceList(InstanceFilter{sort:InstanceSort::ChangedDesc,limit:100,..Default::default()})),
        ("objects_search",QueryRequest::InstanceList(InstanceFilter{search:Some("Windows".into()),limit:100,..Default::default()})),
        ("timeline_routine",QueryRequest::ActionTimeline(ActionFilter{include_routine:true,limit:200,..Default::default()})),
    ];
    if count_mode=="objects" {for (_,request) in &mut cases {
        if let QueryRequest::InstanceList(filter)=request {
            filter.include_counts=false;filter.include_instance_total=filter.cursor.is_none();
        }
    }}
    for (name,request) in cases.clone() {
        if let QueryRequest::InstanceList(mut filter)=request {
            if !filter.include_counts{continue;}
            filter.include_counts=false;
            filter.include_instance_total=true;
            cases.push((match name {"objects_first"=>"objects_first_without_counts","objects_network"=>"objects_network_without_counts","objects_processes"=>"objects_processes_without_counts","objects_recent"=>"objects_recent_without_counts","objects_search"=>"objects_search_without_counts",_=>continue},QueryRequest::InstanceList(filter)));
        }
    }
    let selected=std::env::var("PCSCOPE_PERFORMANCE_CASES").ok();
    let mut complete=true;let mut cases_run=0;
    for (name,request) in cases {
        if selected.as_ref().is_some_and(|value|!value.split(',').any(|part|part==name)){continue;}
        cases_run+=1;
        let mut elapsed=Vec::new();let mut errors=0;
        for _ in 0..samples {let start=Instant::now();match query(store,&request){Ok(answer)=>{
            if matches!(&request,QueryRequest::InstanceList(filter) if filter.include_instance_total){ensure!(answer.instance_total.is_some(),"exact object total missing for {name}");}
            elapsed.push(start.elapsed().as_secs_f64()*1000.0)
        },Err(error)=>{errors+=1;eprintln!("{name}: {error}");if errors>=3{break}}}}
        elapsed.sort_by(f64::total_cmp);
        complete&=elapsed.len()==samples&&errors==0;
        println!("{}",if elapsed.is_empty(){json!({"case":name,"errors":errors})}else{json!({"case":name,"samples":elapsed.len(),"errors":errors,"p50_ms":percentile(&elapsed,0.5),"p95_ms":percentile(&elapsed,0.95),"p99_ms":percentile(&elapsed,0.99),"includes_ipc":false})});
    }ensure!(cases_run>0,"no performance case selected");ensure!(complete,"performance sample collection failed; inspect case errors");Ok(())
}
fn main()->Result<()> {
    let args=std::env::args().skip(1).collect::<Vec<_>>();
    match args.first().map(String::as_str){
        Some("prepare") if args.len()==3=>prepare(Path::new(&args[1]),Path::new(&args[2])),
        Some("live-readonly") if args.len()>=2=>{let store=Store::open_reader(Path::new(&args[1]),Duration::from_secs(1800))?;measure(&store,args.get(2).map(|s|s.parse()).transpose()?.unwrap_or(100))},
        Some("run"|"run-no-anchor") if args.len()>=2=>run(Path::new(&args[1]),args.get(2).map(|s|s.parse()).transpose()?.unwrap_or(100),args[0]=="run"),
        Some("advance") if args.len()>=2=>{
            let store=open(Path::new(&args[1]))?;let slices:usize=args.get(2).map(|s|s.parse()).transpose()?.unwrap_or(100);
            ensure!((1..=100000).contains(&slices),"slices must be 1..100000");let start=Instant::now();
            for count in 1..=slices {let report=store.prepare_display(128,Duration::from_millis(40))?;
                if count%100==0||report["pending"]==false {let status=store.query(&QueryRequest::ProjectionStatus)?.data.unwrap_or_default();
                    println!("{}",json!({"slices":count,"ms":start.elapsed().as_millis(),"display":status.as_array().and_then(|rows|rows.iter().find(|r|r["projection"]=="display")).map(|row|&row["display_state"])}));}
                if report["pending"]==false{break;}
            }Ok(())},
        _=>anyhow::bail!("usage: read-model-performance prepare <read-only-history.db> <existing-target-fixture-dir> | run <fixture-dir> [samples] | advance <fixture-dir>"),
    }
}
