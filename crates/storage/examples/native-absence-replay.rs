//! Controlled prior identity + saved successful native snapshot. Never opens production history.
use anyhow::{ensure,Result};
use pcscope_storage::Store;
use serde_json::{json,Value};
fn drain(store:&Store,machine:&str,installation:&str)->Result<()> {
    loop {let report=store.project_pending(machine,installation,64)?;ensure!(report.rejected==0&&report.unadapted==0,"replay material rejected");if report.examined==0{return Ok(())}}
}
fn main()->Result<()> {
    let fixture:Value=serde_json::from_slice(&std::fs::read("target/native-absence-replay/fixture.json")?)?;
    let root=std::env::current_dir()?.join("target/native-absence-replay").join(format!("{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()));std::fs::create_dir_all(&root)?;
    let mut store=Store::open_program(root.join("history.db"))?;let machine=fixture["machine"].as_str().unwrap();let installation=store.projection_installation(machine)?;
    let before=pcscope_contracts::parse(&fixture["before"].to_string())?;let after=pcscope_contracts::parse(&fixture["after"].to_string())?;
    store.ingest(&before)?;drain(&store,machine,&installation)?;
    let id:i64=store.reader().query_row("SELECT instance_id FROM source_bindings WHERE source='pcscope.hardware.devices' AND item_id=? LIMIT 1",[fixture["id"].as_str().unwrap()],|row|row.get(0))?;
    let gap=pcscope_contracts::parse(&json!({"protocol_version":1,"request_id":"replay-gap","type":"gap","source":"pcscope.hardware.devices","observed_at":fixture["after"]["observed_at"],"reason":"controlled unavailable provider; not absence"}).to_string())?;
    store.ingest(&gap)?;drain(&store,machine,&installation)?;
    let after_gap:String=store.reader().query_row("SELECT lifecycle FROM object_instances WHERE instance_id=?",[id],|row|row.get(0))?;ensure!(after_gap=="active","source gap changed object presence");
    store.ingest(&after)?;drain(&store,machine,&installation)?;
    let lifecycle:String=store.reader().query_row("SELECT lifecycle FROM object_instances WHERE instance_id=?",[id],|row|row.get(0))?;
    let raw_changes:i64=store.reader().query_row("SELECT count(*) FROM records WHERE source='pcscope.hardware.devices' AND kind='object.no_longer_observed' AND object_id=?",[fixture["id"].as_str().unwrap()],|row|row.get(0))?;
    let actions:i64=store.reader().query_row("SELECT count(*) FROM object_actions WHERE instance_id=? AND action_id='observed.no_longer_observed'",[id],|row|row.get(0))?;
    let timeline=store.query(&pcscope_contracts::query::QueryRequest::ActionTimeline(pcscope_contracts::query::ActionFilter{source:Some("pcscope.hardware.devices".into()),action_id:Some("observed.no_longer_observed".into()),..Default::default()}))?;
    ensure!(timeline.data.as_ref().and_then(Value::as_array).is_some_and(|rows|rows.len()==1),"absence not returned by the UI timeline query");
    let summary=json!({"data":root,"instance_id":id,"lifecycle":lifecycle,"raw_absence_changes":raw_changes,"typed_absence_actions":actions,"ui_timeline":timeline,"gap_kept_active":true,"provenance":fixture["provenance"],"production_history_accessed":false,"device_modified":false});std::fs::write(root.join("summary.json"),serde_json::to_vec_pretty(&summary)?)?;println!("{summary}");
    ensure!(raw_changes==1,"native snapshot difference did not record absence");ensure!(lifecycle=="not_observed"&&actions==1,"absence did not reach current object and typed timeline");Ok(())
}
