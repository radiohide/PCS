//! Failed channels cool down independently; one denied log cannot slow healthy channels.
use serde_json::{json,Value};
use std::{collections::BTreeMap,time::{Duration,Instant}};
#[derive(Default)]
pub struct Backoff {entries:BTreeMap<String,Entry>}
struct Entry {failures:u32,next:Instant,result:Value}
impl Backoff {
    pub fn cooling(&self,channel:&str,manual:bool)->Option<Value> {
        if manual {return None;}
        self.entries.get(channel).filter(|entry|entry.next>Instant::now()).map(|entry| {
            let mut result=entry.result.clone();result["cooling_down"]=json!(true);result["retry_in_seconds"]=json!(entry.next.saturating_duration_since(Instant::now()).as_secs());result
        })
    }
    pub fn record(&mut self,channel:&str,result:&mut Value) {
        result["last_attempt_at"]=json!(chrono::Utc::now().to_rfc3339());
        if result["ok"]==true {self.entries.remove(channel);return;}
        let failures=self.entries.get(channel).map_or(1,|entry|entry.failures.saturating_add(1));
        let seconds=60u64.saturating_mul(1u64<<failures.saturating_sub(1).min(4)).min(900);
        result["retry_delay_seconds"]=json!(seconds);result["consecutive_failures"]=json!(failures);
        self.entries.insert(channel.into(),Entry{failures,next:Instant::now()+Duration::from_secs(seconds),result:result.clone()});
    }
}
#[cfg(test)]mod tests {
    use super::*;
    #[test]fn denied_channel_retries_are_bounded_without_stopping_healthy_logs() {
        let mut backoff=Backoff::default();let mut failed=json!({"ok":false,"gaps":["denied"],"history_state":"gap"});
        for delay in [60,120,240,480,900,900] {backoff.record("Security",&mut failed);assert_eq!(failed["retry_delay_seconds"],delay);assert_eq!(backoff.cooling("Security",false).unwrap()["history_state"],"gap");assert!(backoff.cooling("Application",false).is_none());assert!(backoff.cooling("Security",true).is_none());}
        let mut success=json!({"ok":true});backoff.record("Security",&mut success);assert!(backoff.cooling("Security",false).is_none());
    }
}
