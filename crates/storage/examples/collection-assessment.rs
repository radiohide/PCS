//! Fresh isolated on-disk audit and benchmark. Never starts a daemon or opens production data.
//! cargo run --release -p pcscope-storage --example collection-assessment -- <summary.json>
use anyhow::{ensure, Context, Result};
use pcscope_contracts::{query::{ActionFilter, InstanceFilter, QueryRequest}, Envelope};
use pcscope_storage::Store;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::{Path, PathBuf}, time::Instant};

fn count(store: &Store, table: &str) -> Result<i64> {
    Ok(store.reader().query_row(&format!("SELECT count(*) FROM {table}"), [], |r|r.get(0))?)
}
fn footprint(store: &Store, path: &Path) -> Result<Value> {
    let live: i64=store.reader().query_row("SELECT page_count*page_size FROM pragma_page_count(),pragma_page_size()", [], |r|r.get(0))?;
    let files: Vec<u64>=["", "-wal", "-shm"].iter().map(|suffix|std::fs::metadata(format!("{}{suffix}",path.display())).map_or(0,|m|m.len())).collect();
    Ok(json!({"allocated_database_bytes":live,"db_bytes":files[0],"wal_bytes":files[1],"shm_bytes":files[2],"physical_bytes":files.iter().sum::<u64>()}))
}
fn drain(store: &Store) -> Result<Value> {
    let start=Instant::now();let mut examined=0;let mut projected=0;let mut skipped=0;let mut unadapted=0;let mut rejected=0;
    loop {let r=store.project_pending("assessment-machine","assessment-install",1000)?;examined+=r.examined;projected+=r.projected;skipped+=r.skipped;unadapted+=r.unadapted;rejected+=r.rejected;if r.examined==0{break}}
    Ok(json!({"seconds":start.elapsed().as_secs_f64(),"examined":examined,"projected":projected,"skipped":skipped,"unadapted":unadapted,"rejected":rejected}))
}
fn queries(store: &Store, path: &Path) -> Result<Value> {
    let requests=vec![
        ("instance_first_page",QueryRequest::InstanceList(InstanceFilter::default())),
        ("instance_no_match",QueryRequest::InstanceList(InstanceFilter{search:Some("assessment-no-match-xyz".into()),..Default::default()})),
        ("action_first_page",QueryRequest::ActionTimeline(ActionFilter::default())),
        ("resource_readings",QueryRequest::SourceReadings{source:Some("pcscope.execution.resources".into()),cursor:None,limit:50}),
        ("source_coverage",QueryRequest::SourceCoverage{source:None}),
    ];
    let mut results=json!({});
    for (name,request) in requests {
        let fresh=Store::open(path)?;let start=Instant::now();let response=fresh.query(&request)?;let fresh_ms=start.elapsed().as_secs_f64()*1000.;drop(fresh);
        let mut samples=Vec::new();
        for _ in 0..30 {let start=Instant::now();let _answer=store.query(&request)?;samples.push(start.elapsed().as_secs_f64()*1000.)}
        samples.sort_by(f64::total_cmp);
        results[name]=json!({"fresh_connection_ms":fresh_ms,"warm_p50_ms":samples[14],"warm_p95_ms":samples[28],"warm_max_ms":samples[29],"repetitions":30,"status":response.status});
    }
    Ok(results)
}
fn synthetic(batch: usize, round: usize) -> Result<Envelope> {
    let items:Vec<Value>=(0..200).map(|i|{let id=format!("\\assessment\\task-{:05}",batch*200+i);json!({"id":id,"data":{"path":id,"name":format!("assessment-task-{:05}",batch*200+i),"enabled":true,"principal":"assessment","actions":[],"triggers":[]}})}).collect();
    pcscope_contracts::parse(&json!({"protocol_version":1,"request_id":format!("synthetic/{batch}/{round}"),"type":"snapshot","source":"pcscope.startup.tasks","scope":format!("machine:assessment/schema:1/context:batch-{batch}"),"run_id":format!("round-{round}"),"observed_at":format!("2026-10-12T03:{:02}:00Z",round%60),"complete":true,"items":items}).to_string())
}
fn comparable(value: &Value) -> Result<Value> {
    if value.get("sensitive")==Some(&Value::Bool(true)){return comparable(&pcscope_contracts::sensitive::unprotect(value)?)}
    // A 16 KiB display preview is not the source field; both backends retain its full value.
    if value.get("preview").is_some() {if let Some(full)=value.get("full_value"){return comparable(full)}}
    Ok(match value {
        Value::Array(values)=>Value::Array(values.iter().map(comparable).collect::<Result<_>>()?),
        Value::Object(values)=>Value::Object(values.iter().map(|(key,value)|Ok((key.clone(),comparable(value)?))).collect::<Result<_>>()?),
        _=>value.clone(),
    })
}
fn compare_eventlog(manifest: &Path) -> Result<()> {
    let result:Value=serde_json::from_slice(&std::fs::read(manifest)?)?;
    let mut collections=Vec::new();let mut xml_checked=0;let mut xml_failed=0;let mut xml_mismatch=0;
    for backend in result["results"].as_array().context("backends")? {
        let mut events=BTreeMap::new();let mut duplicates=0;
        for line in std::fs::read_to_string(backend["file"].as_str().context("backend file")?)?.lines() {
            let envelope=pcscope_contracts::parse(line)?;
            if let pcscope_contracts::Message::EventBatch{events:batch,..}=envelope.message {
                for event in batch {
                    if events.contains_key(&event.id){duplicates+=1}
                    match pcscope_contracts::sensitive::unprotect(&event.data["raw_xml"]) {Ok(value)=>{if let Some(xml)=value.as_str(){xml_checked+=1;if Some(format!("{:x}",Sha256::digest(xml.as_bytes())).as_str())!=event.data["xml_sha256"].as_str(){xml_mismatch+=1}}else{xml_failed+=1}},Err(_)=>xml_failed+=1}
                    events.insert(event.id.clone(),event);
                }
            }
        }
        collections.push((events,duplicates));
    }
    ensure!(collections.len()==2,"two backends required");let a=&collections[0].0;let b=&collections[1].0;
    let mut missing=0;let mut hash=0;let mut kind=0;let mut fields=0;let mut system=0;let mut field_differences=Vec::new();let mut representation_differences=0;
    for (id,event) in a {
        if let Some(other)=b.get(id) {
            if event.data["xml_sha256"]!=other.data["xml_sha256"] {hash+=1}
            if event.kind!=other.kind {kind+=1}
            let left=comparable(&event.data["fields"])?;let right=comparable(&other.data["fields"])?;
            if !pcscope_contracts::sensitive::equivalent(&event.data["fields"],&other.data["fields"]){representation_differences+=1}
            if left!=right {fields+=1;if field_differences.len()<10 {let keys:Vec<_>=left.as_object().into_iter().flat_map(|m|m.keys()).filter(|k|left.get(*k)!=right.get(*k)).cloned().collect();field_differences.push(json!({"record_id":id,"event_id":event.data["event_id"],"differing_keys":keys}));}}
            if event.data["system"]!=other.data["system"] {system+=1}
        } else {missing+=1}
    }
    missing+=b.keys().filter(|id|!a.contains_key(*id)).count();
    let result=json!({"events":[a.len(),b.len()],"duplicate_ids":[collections[0].1,collections[1].1],"id_differences":missing,"xml_hash_differences":hash,"kind_differences":kind,"field_value_differences_after_decryption":fields,"field_differences":field_differences,"field_representation_differences":representation_differences,"system_differences":system,"xml_checked":xml_checked,"xml_decrypt_failed":xml_failed,"xml_hash_mismatches":xml_mismatch,"policy":"compare decrypted full values; ciphertext, display previews and additional encryption are representation differences"});
    std::fs::write(manifest.parent().unwrap().join("comparison-verified.json"),serde_json::to_vec_pretty(&result)?)?;println!("{result}");Ok(())
}
fn main() -> Result<()> {
    let argument=std::env::args().nth(1).context("summary.json or compare-eventlog <result.json> required")?;
    if argument=="compare-eventlog" {return compare_eventlog(Path::new(&std::env::args().nth(2).context("result.json required")?))}
    let manifest=PathBuf::from(argument).canonicalize()?;
    let canonical_root=manifest.parent().unwrap();ensure!(canonical_root.components().any(|p|p.as_os_str()=="target"),"only ignored target assessment data permitted");
    // SQLite on this Windows build accepts regular absolute paths, not canonicalize's verbatim prefix.
    let root_text=canonical_root.to_string_lossy();let root=PathBuf::from(root_text.strip_prefix(r"\\?\").unwrap_or(&root_text));
    let summary:Value=serde_json::from_slice(&std::fs::read(&manifest)?)?;
    let path=root.join(format!("assessment-real-{}.db",std::process::id()));ensure!(!path.exists(),"fresh database required");
    let mut store=Store::open(&path)?;let mut envelopes=Vec::new();let mut parse_failures=Vec::new();let mut rejected=Vec::new();let mut expected=BTreeMap::new();let mut input_bytes=0;
    let mut ingest_seconds=0.;
    for job in summary["jobs"].as_array().context("jobs")? {
        let text=std::fs::read_to_string(job["file"].as_str().context("file")?)?;input_bytes+=text.len();
        for (line_number,line) in text.lines().filter(|l|!l.trim().is_empty()).enumerate() {
            match pcscope_contracts::parse(line) {
                Ok(envelope)=>{
                    let start=Instant::now();let result=store.ingest(&envelope);ingest_seconds+=start.elapsed().as_secs_f64();
                    match result {Ok(_)=>{for input in pcscope_contracts::reference::evidence_from(&envelope){expected.insert((input.evidence.id,input.item_id),input.payload);}envelopes.push(envelope)},Err(error)=>rejected.push(json!({"job":job["job"],"line":line_number+1,"error":error.to_string()}))}
                }
                Err(error)=>parse_failures.push(json!({"job":job["job"],"line":line_number+1,"error":error.to_string()})),
            }
        }
    }
    let projection=drain(&store)?;
    let start=Instant::now();let stored=pcscope_storage::evidence::inputs(store.reader(),0,expected.len()+1)?;
    let mut mismatches=0;let mut unexpected=0;
    for input in &stored {match expected.get(&(input.evidence.id.clone(),input.item_id.clone())) {Some(payload) if payload==&input.payload=>{},Some(_)=>mismatches+=1,None=>unexpected+=1}}
    let verification_seconds=start.elapsed().as_secs_f64();
    let mut xml_checked=0;let mut xml_failed=0;let mut xml_hash_mismatch=0;
    for input in &stored {if let Some(raw)=input.payload.get("raw_xml") {match pcscope_contracts::sensitive::unprotect(raw) {Ok(value)=>{if let Some(xml)=value.as_str(){xml_checked+=1;let digest=format!("{:x}",Sha256::digest(xml.as_bytes()));if Some(digest.as_str())!=input.payload["xml_sha256"].as_str(){xml_hash_mismatch+=1}}else{xml_failed+=1}},Err(_)=>xml_failed+=1}}}
    let counts=json!({"evidence":count(&store,"evidence")?,"instances":count(&store,"object_instances")?,"facts":count(&store,"object_facts")?,"actions":count(&store,"object_actions")?,"records":count(&store,"records")?,"snapshots":count(&store,"snapshots")?});
    let mut replay_seconds=0.;let mut replay_rejections=0;
    for envelope in &envelopes {let start=Instant::now();if store.ingest(envelope).is_err(){replay_rejections+=1};replay_seconds+=start.elapsed().as_secs_f64()}
    let replay=drain(&store)?;
    let failed:Vec<Value>={let mut statement=store.reader().prepare("SELECT e.source,q.last_error,count(*) FROM projection_queue q JOIN evidence e ON e.evidence_id=q.evidence_id AND e.item_id=q.item_id WHERE q.state='failed' GROUP BY e.source,q.last_error")?;let rows=statement.query_map([],|row|Ok(json!({"source":row.get::<_,String>(0)?,"reason":row.get::<_,Option<String>>(1)?,"count":row.get::<_,i64>(2)?})))?;rows.collect::<rusqlite::Result<_>>()?};
    let real=json!({"profile":if cfg!(debug_assertions){"debug"}else{"release"},"input_bytes":input_bytes,"accepted_messages":envelopes.len(),"parse_failures":parse_failures,"ingest_rejections":rejected,"ingest_seconds":ingest_seconds,"projection":projection,"counts":counts,"payload_verification":{"expected":expected.len(),"stored":stored.len(),"mismatches":mismatches,"unexpected":unexpected,"seconds":verification_seconds},"xml_verification":{"checked":xml_checked,"decrypt_failed":xml_failed,"hash_mismatches":xml_hash_mismatch},"replay_seconds":replay_seconds,"replay_rejections":replay_rejections,"replay_projection":replay,"projection_failures":failed,"queries":queries(&store,&path)?,"footprint":footprint(&store,&path)?});
    println!("{}",json!({"stage":"real","result":real}));
    let scale_path=root.join(format!("assessment-synthetic-{}.db",std::process::id()));ensure!(!scale_path.exists(),"fresh synthetic database required");let mut scale=Store::open(&scale_path)?;
    let mut stages=Vec::new();
    for (from,to) in [(0,10),(10,50)] {
        let start=Instant::now();for batch in from..to{scale.ingest(&synthetic(batch,0)?)?;}let ingest_ms=start.elapsed().as_secs_f64()*1000.;let projection=drain(&scale)?;
        let result=json!({"instances":count(&scale,"object_instances")?,"evidence":count(&scale,"evidence")?,"added_items":(to-from)*200,"ingest_ms":ingest_ms,"projection":projection,"queries":queries(&scale,&scale_path)?,"footprint":footprint(&scale,&scale_path)?});
        println!("{}",json!({"stage":"synthetic","result":result}));stages.push(result);
    }
    let before=footprint(&scale,&scale_path)?;let facts_before=count(&scale,"object_facts")?;let actions_before=count(&scale,"object_actions")?;
    let start=Instant::now();for round in 1..=20 {scale.ingest(&synthetic(0,round)?)?;let _=drain(&scale)?;}
    let unchanged=json!({"rounds":20,"items_per_round":200,"seconds":start.elapsed().as_secs_f64(),"footprint_before":before,"footprint_after":footprint(&scale,&scale_path)?,"facts_before":facts_before,"facts_after":count(&scale,"object_facts")?,"actions_before":actions_before,"actions_after":count(&scale,"object_actions")?});
    let integrity:String=scale.reader().query_row("PRAGMA integrity_check",[],|r|r.get(0))?;ensure!(integrity=="ok","synthetic integrity check");
    let integrity:String=store.reader().query_row("PRAGMA integrity_check",[],|r|r.get(0))?;ensure!(integrity=="ok","real integrity check");
    let result=json!({"real":real,"synthetic":stages,"unchanged":unchanged,"integrity":"ok","query_scope":"in-process Store query; fresh connection is not cold OS cache; excludes IPC/UI"});
    std::fs::write(root.join("storage-assessment.json"),serde_json::to_vec_pretty(&result)?)?;
    println!("{}",json!({"stage":"complete","unchanged":unchanged,"report":root.join("storage-assessment.json")}));
    Ok(())
}
