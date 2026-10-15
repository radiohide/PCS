//! Offline, read-only audit through the same timeline query used by the desktop.
use anyhow::{ensure,Result};
use pcscope_contracts::query::{ActionFilter,QueryRequest};
use pcscope_storage::Store;
use std::{io::{BufWriter,Write},time::Duration};
fn main()->Result<()> {
    let args:Vec<String>=std::env::args().collect();
    ensure!(args.len()==3,"usage: history_audit frozen.db output.jsonl");
    let store=Store::open_reader(&args[1],Duration::from_secs(1800))?;
    // Never replace an existing export or allow the output to truncate the input database.
    let mut output=BufWriter::new(std::fs::OpenOptions::new().write(true).create_new(true).open(&args[2])?);
    let mut cursor=None;let mut count=0;
    loop {
        let response=store.query(&QueryRequest::ActionTimeline(ActionFilter{limit:200,cursor,..Default::default()}))?;
        let rows=response.data.as_ref().and_then(|v|v.as_array()).ok_or_else(||anyhow::anyhow!("query failed: {:?}",response))?;
        for row in rows {serde_json::to_writer(&mut output,row)?;writeln!(output)?;}
        count+=rows.len();
        let page=response.page.ok_or_else(||anyhow::anyhow!("no pagination"))?;
        if !page.has_more {break;}
        cursor=page.next_cursor;
        if count%10000==0 {eprintln!("{count} actions audited");}
    }
    output.flush()?;eprintln!("complete: {count} actions");Ok(())
}
