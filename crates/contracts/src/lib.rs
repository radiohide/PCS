use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub mod adapter;
pub mod adapters;
pub mod identity;
pub mod event_state;
pub mod model;
pub mod query;
pub mod reference;
pub mod semantics;
pub mod sensitive;

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_LINE_BYTES: usize = 1_048_576;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    pub protocol_version: u32,
    pub request_id: String,
    #[serde(flatten)]
    pub message: Message,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    EventBatch {
        source: String,
        epoch: String,
        batch_seq: u64,
        checkpoint: String,
        events: Vec<Event>,
    },
    Snapshot {
        source: String,
        scope: String,
        run_id: String,
        observed_at: String,
        complete: bool,
        items: Vec<Item>,
    },
    Gap {
        source: String,
        reason: String,
        observed_at: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub id: String,
    pub occurred_at: String,
    pub kind: String,
    pub object_id: String,
    pub data: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub id: String,
    pub data: Value,
}

pub fn parse(line: &str) -> Result<Envelope> {
    if line.len() > MAX_LINE_BYTES {
        bail!("message exceeds 1 MiB limit");
    }
    let envelope: Envelope = serde_json::from_str(line)?;
    if envelope.protocol_version != PROTOCOL_VERSION {
        bail!("unsupported protocol version");
    }
    if envelope.request_id.is_empty() {
        bail!("request_id must not be empty");
    }
    match &envelope.message {
        Message::EventBatch {
            source,
            epoch,
            batch_seq,
            events,
            ..
        } => {
            if source.is_empty() || epoch.is_empty() {
                bail!("source and epoch required");
            }
            if *batch_seq == 0 || *batch_seq > i64::MAX as u64 {
                bail!("batch_seq must be a positive signed 64-bit integer");
            }
            let mut ids = std::collections::HashSet::new();
            for event in events {
                if event.id.is_empty() || event.kind.is_empty() || !ids.insert(&event.id) {
                    bail!("empty or duplicate event identity");
                }
            }
        }
        Message::Snapshot {
            source,
            scope,
            run_id,
            items,
            ..
        } => {
            if source.is_empty() || scope.is_empty() || run_id.is_empty() {
                bail!("snapshot identity required");
            }
            let mut ids = std::collections::HashSet::new();
            for item in items {
                if item.id.is_empty() || !ids.insert(&item.id) {
                    bail!("empty or duplicate item identity");
                }
            }
        }
        Message::Gap { source, reason, .. } => {
            if source.is_empty() || reason.is_empty() {
                bail!("gap source/reason required");
            }
        }
    }
    Ok(envelope)
}
