use crate::*;

impl Store {
    /// Explicit fresh-instance setup. Never resets or imports an existing history.
    pub fn initialize_collection_baseline(&mut self, seconds: u64) -> Result<Value> {
        anyhow::ensure!((30..=604800).contains(&seconds), "采集周期应为 30–604800 秒");
        let tx = self.conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let populated: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM evidence) OR EXISTS(SELECT 1 FROM records)
             OR EXISTS(SELECT 1 FROM snapshots)
             OR EXISTS(SELECT 1 FROM settings WHERE key IN ('collection_baseline','plugin_schedule'))",
            [], |r| r.get(0),
        )?;
        anyhow::ensure!(!populated, "只能初始化空数据库；已有记录和设置保持不变");
        let started_at: String = tx.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ','now')", [], |r| r.get(0))?;
        let baseline = json!({"version":1,"started_at":started_at,"initial_seconds":seconds,"event_history":"from_baseline"});
        tx.execute("INSERT INTO settings VALUES('collection_baseline',?)", [baseline.to_string()])?;
        tx.execute("INSERT INTO settings VALUES('plugin_schedule',?)", [json!({"_global_seconds":seconds,"_runtime_seconds":seconds}).to_string()])?;
        tx.commit()?;
        Ok(baseline)
    }

    pub fn collection_started_at(&self) -> Result<Option<String>> {
        let value: Option<String> = self.conn.query_row("SELECT value FROM settings WHERE key='collection_baseline'", [], |r| r.get(0)).optional()?;
        value.map(|v| {
            let parsed: Value = serde_json::from_str(&v)?;
            anyhow::ensure!(parsed["version"] == 1 && parsed["event_history"] == "from_baseline", "未知的采集基线设置");
            parsed["started_at"].as_str().filter(|s| !s.is_empty()).map(str::to_owned).context("采集起点缺失")
        }).transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn baseline_is_explicit_atomic_and_cannot_replace_existing_history_or_configuration() {
        let mut store = Store::open(":memory:").unwrap();
        assert_eq!(store.collection_started_at().unwrap(), None);
        assert!(store.initialize_collection_baseline(10).is_err());
        assert_eq!(store.plugin_settings().unwrap(), json!({}));
        let baseline = store.initialize_collection_baseline(600).unwrap();
        assert_eq!(store.collection_started_at().unwrap().unwrap(), baseline["started_at"]);
        assert_eq!(store.plugin_settings().unwrap(), json!({"_global_seconds":600,"_runtime_seconds":600}));
        assert!(store.initialize_collection_baseline(60).is_err());
        assert_eq!(store.collection_started_at().unwrap().unwrap(), baseline["started_at"]);
        let mut used = Store::open(":memory:").unwrap();
        used.begin_session("2026-10-22T00:00:00Z").unwrap();
        assert!(used.initialize_collection_baseline(600).is_err());
        assert_eq!(used.collection_started_at().unwrap(), None);
        let mut configured = Store::open(":memory:").unwrap();
        configured.save_plugin_settings(&json!({"_global_seconds":120})).unwrap();
        assert!(configured.initialize_collection_baseline(600).is_err());
        assert_eq!(configured.plugin_settings().unwrap()["_global_seconds"],120);
    }
}
