//! Replays real authorized native-wrapper output into a new isolated database, never production.
use anyhow::{ensure,Result};
use pcscope_contracts::{parse,Message,query::{InstanceFilter,QueryRequest}};
use pcscope_storage::Store;
use serde_json::{json,Value};
use std::collections::BTreeMap;
fn main()->Result<()> {
    let request_text=std::fs::read_to_string("target/native-five-source-wrapper-request.json")?;
    let request:Value=serde_json::from_str(request_text.trim_start_matches('\u{feff}'))?;
    let input=std::path::Path::new(request["root"].as_str().unwrap()).join("enhanced");
    let root=std::env::current_dir()?.join("target/native-five-source-projection").join(format!("{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()));std::fs::create_dir_all(&root)?;
    let mut store=Store::open_program(root.join("history.db"))?;
    let mut machine=None;let mut counts=BTreeMap::<String,usize>::new();let mut envelopes=0;
    for unit in ["system","security"] {
        for line in std::fs::read_to_string(input.join(format!("{unit}.jsonl")))?.lines().filter(|line|!line.trim().is_empty()) {
            let envelope=parse(line)?;
            if let Message::Snapshot{source,scope,complete,items,..}=&envelope.message {
                ensure!(*complete,"incomplete acceptance material");
                let observed_machine=scope.split('/').next().unwrap().strip_prefix("machine:").unwrap();
                if let Some(previous)=&machine {ensure!(previous==observed_machine,"mixed machine identity");}else{machine=Some(observed_machine.to_string());}
                *counts.entry(source.clone()).or_default()+=items.len();
            }else{anyhow::bail!("acceptance material must contain only complete snapshots")}
            store.ingest(&envelope)?;envelopes+=1;
        }
    }
    ensure!(envelopes==80&&counts.len()==5,"expected five complete 16-shard sources");
    let machine=machine.unwrap();let installation=store.projection_installation(&machine)?;
    loop {let report=store.project_pending(&machine,&installation,64)?;ensure!(report.rejected==0&&report.unadapted==0,"native material did not adapt");if report.examined==0{break}}
    let types=[("pcscope.software.optional-features","software.optional_feature"),("pcscope.software.provisioned","software.provisioned_package"),("pcscope.security.secureboot","security.secure_boot_configuration"),("pcscope.security.encryption","security.bitlocker_configuration"),("pcscope.security.tpm","hardware.tpm")];
    let mut results=vec![];
    for (source,type_id) in types {
        let bound:i64=store.reader().query_row("SELECT count(DISTINCT instance_id) FROM source_bindings WHERE source=?",[source],|row|row.get(0))?;
        ensure!(bound as usize==counts[source],"native source lost object identities: {source}");
        let query=store.query(&QueryRequest::InstanceList(InstanceFilter{source:Some(source.into()),type_id:Some(type_id.into()),..Default::default()}))?;
        ensure!(query.counts.as_ref().is_some_and(|value|value.instances as usize==counts[source]),"UI object query lost a native source: {source}");
        results.push(json!({"source":source,"type_id":type_id,"items":counts[source],"bound_objects":bound,"ui_query":query.status}));
    }
    let failed:i64=store.reader().query_row("SELECT count(*) FROM projection_queue WHERE state IN ('pending','failed')",[],|row|row.get(0))?;ensure!(failed==0,"pending or failed native projection work");
    let summary=json!({"input":input,"data":root,"sources":results,"envelopes":envelopes,"pending_or_failed":failed,"formal_database_accessed":false,"system_changes":false,"collection_in_this_program":false});
    std::fs::write(root.join("summary.json"),serde_json::to_vec_pretty(&summary)?)?;println!("{summary}");Ok(())
}
