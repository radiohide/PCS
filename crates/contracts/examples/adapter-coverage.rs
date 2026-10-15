//! S21: writes what the adapters declare, so the reconciliation compares a machine-readable list
//! against the source inventory instead of against a document someone maintained by hand.
//!
//! usage: adapter-coverage [output-path]
use pcscope_contracts::adapters;
use serde_json::json;

fn main() -> anyhow::Result<()> {
    let output = std::env::args().nth(1).unwrap_or_else(|| "model/adapter-coverage.json".into());
    let declared: Vec<serde_json::Value> = adapters::all_specs()
        .map(|spec| {
            json!({
                "source": spec.source,
                // The type a source produces when it produces exactly one; a source that decides per
                // item (the event adapter, the file tree) states none here and names them in its map.
                "type_id": spec.type_id,
            })
        })
        .collect();
    let document = json!({
        "generated_by": "crates/contracts/examples/adapter-coverage.rs",
        "note": "已声明来源的清单。对账把它与 model/source-map.cjs 和 model/sources.json 比对；差异必须由 model/adapter-exceptions.cjs 逐条给出原因。",
        "event_prefix": "pcscope.eventlog.",
        "usn_prefix": "pcscope.usn.",
        // The rule's own parameters, so the check applies this rule rather than a copy of it.
        "event_exclusions": adapters::event::EVENT_EXCLUSIONS,
        "event_sources": adapters::event::EVENT_SOURCES,
        "reference_sources": ["pcscope.startup.tasks", "pcscope.startup.task-details", "pcscope.execution.processes"],
        "declared": declared,
    });
    std::fs::write(&output, serde_json::to_string_pretty(&document)? + "\n")?;
    println!("adapter coverage: {} declared sources, written to {}", document["declared"].as_array().map_or(0, Vec::len), output);
    Ok(())
}
