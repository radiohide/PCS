//! Replay freshly collected source fixtures into an in-memory store; no daemon or formal data.
use pcscope_contracts::query::{ActionFilter,InstanceFilter,QueryRequest};
use pcscope_storage::Store;
fn main()->anyhow::Result<()> {
    let mut store=Store::open(":memory:")?;let mut messages=0;let mut gaps=0;let mut envelopes=Vec::new();
    for file in std::env::args().skip(1) {
        for line in std::fs::read_to_string(file)?.lines().filter(|line|!line.trim().is_empty()) {
            let envelope=pcscope_contracts::parse(line)?;
            if matches!(envelope.message,pcscope_contracts::Message::Gap{..}) {gaps+=1}
            store.ingest(&envelope)?;envelopes.push(envelope);messages+=1;
        }
    }
    loop {let report=store.project_pending("audit-machine","audit-install",1000)?;anyhow::ensure!(report.unadapted==0 && report.rejected==0,"source projection failed: {report:?}");if report.examined==0{break}}
    let objects=store.query(&QueryRequest::InstanceList(InstanceFilter{limit:200,..Default::default()}))?;
    let mut update_filter=ActionFilter{source:Some("pcscope.system.update-history".into()),limit:200,..Default::default()};let mut updates=0;
    loop {let answer=store.query(&QueryRequest::ActionTimeline(update_filter.clone()))?;updates+=answer.data.as_ref().and_then(|value|value.as_array()).map_or(0,Vec::len);let page=answer.page.unwrap();if !page.has_more{break};update_filter.cursor=page.next_cursor;}
    let samples=store.query(&QueryRequest::SourceReadings{source:Some("pcscope.execution.resources".into()),cursor:None,limit:200})?;
    let before:i64=store.reader().query_row("SELECT count(*) FROM object_actions",[],|row|row.get(0))?;
    for envelope in &envelopes {store.ingest(envelope)?;}
    let replay=store.project_pending("audit-machine","audit-install",1000)?;anyhow::ensure!(replay.examined==0,"duplicate evidence queued new work");
    let after:i64=store.reader().query_row("SELECT count(*) FROM object_actions",[],|row|row.get(0))?;anyhow::ensure!(before==after,"replay duplicated actions");
    println!("{}",serde_json::json!({"messages":messages,"collection_gaps":gaps,"instances":objects.counts,"update_actions":updates,"samples":samples.counts,"source_projection_failures":0,"replay_duplicate_work":replay.examined}));
    Ok(())
}
