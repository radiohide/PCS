use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

pub struct Job {
    pub id: String,
    pub events: bool,
    pub enabled: bool,
    pub seconds: u64,
    next: Instant,
    phase: &'static str,
    manual: bool,
    started: Option<String>,
    finished: Option<String>,
    began: Option<Instant>,
    result: Value,
    failures: u32,
    manual_after: Instant,
    refresh_requested: bool,
}
pub struct Schedule {
    pub jobs: Vec<Job>,
    busy: [bool; 2],
    global_seconds: Option<u64>,
    pub runtime_seconds: Option<u64>,
}
impl Job {
    fn due_at(&self)->Instant {if self.refresh_requested&&self.failures==0 {self.next.min(self.manual_after)}else{self.next}}
}
impl Schedule {
    pub fn new(ids: Vec<String>, saved: &Value) -> Result<Self> {
        let global_seconds = saved["_global_seconds"].as_u64();
        let runtime_seconds = saved["_runtime_seconds"].as_u64();
        for seconds in [global_seconds, runtime_seconds].into_iter().flatten() { Self::validate(false, seconds)?; }
        let mut jobs = Vec::new();
        for id in ids.into_iter().chain(["eventlog".into(), "usn".into()]) {
            let events = id == "eventlog" || id == "usn";
            let enabled = saved[&id]["enabled"].as_bool().unwrap_or(true);
            let seconds = saved[&id]["seconds"]
                .as_u64()
                .unwrap_or(global_seconds.unwrap_or(if events { 10 } else { 300 }));
            Self::validate(events, seconds)?;
            jobs.push(Job {
                id,
                events,
                enabled,
                seconds,
                next: Instant::now(),
                phase: "idle",
                manual: false,
                started: None,
                finished: None,
                began: None,
                result: Value::Null,
                failures: 0,
                manual_after: Instant::now(),
                refresh_requested: false,
            });
        }
        Ok(Self {
            jobs,
            busy: [false, false],
            global_seconds,
            runtime_seconds,
        })
    }
    fn validate(events: bool, seconds: u64) -> Result<()> {
        if !(if events { 10 } else { 30 }..=604800).contains(&seconds) {
            bail!("周期范围：状态采集 30–604800 秒，日志采集 10–604800 秒");
        }
        Ok(())
    }
    pub fn config(&self) -> Value {
        let mut map = serde_json::Map::new();
        if let Some(seconds) = self.global_seconds { map.insert("_global_seconds".into(), json!(seconds)); }
        if let Some(seconds) = self.runtime_seconds { map.insert("_runtime_seconds".into(), json!(seconds)); }
        for j in &self.jobs {
            map.insert(
                j.id.clone(),
                json!({"enabled":j.enabled,"seconds":j.seconds}),
            );
        }
        Value::Object(map)
    }
    pub fn candidate(&self, id: &str, enabled: bool, seconds: u64) -> Result<Value> {
        let j = self
            .jobs
            .iter()
            .find(|j| j.id == id)
            .ok_or_else(|| anyhow::anyhow!("未知插件"))?;
        Self::validate(j.events, seconds)?;
        let mut value = self.config();
        value[id] = json!({"enabled":enabled,"seconds":seconds});
        if id == "scoped" { value["_runtime_seconds"] = json!(seconds); }
        Ok(value)
    }
    pub fn global_view(&self) -> Value {
        json!({"seconds":self.global_seconds,"minimum_seconds":30,"maximum_seconds":604800})
    }
    pub fn global_candidate(&self, seconds: u64) -> Result<Value> {
        Self::validate(false, seconds)?;
        let mut config = self.config();
        for job in &self.jobs { config[&job.id]["seconds"] = json!(seconds); }
        config["_global_seconds"] = json!(seconds);
        config["_runtime_seconds"] = json!(seconds);
        Ok(config)
    }
    pub fn configure_global(&mut self, seconds: u64) {
        self.global_seconds = Some(seconds);
        self.runtime_seconds = Some(seconds);
        let next = Instant::now() + Duration::from_secs(seconds);
        for job in &mut self.jobs { job.seconds = seconds; job.next = next; }
    }
    pub fn configure(&mut self, id: &str, enabled: bool, seconds: u64) {
        if id == "scoped" { self.runtime_seconds = Some(seconds); }
        let j = self.jobs.iter_mut().find(|j| j.id == id).unwrap();
        j.enabled = enabled;
        j.seconds = seconds;
        j.next = Instant::now() + Duration::from_secs(seconds);
        if !enabled {j.refresh_requested=false;}
    }
    pub fn request_refresh(&mut self,id:&str) {
        if let Some(job)=self.jobs.iter_mut().find(|job|job.id==id&&job.enabled&&!job.events){job.refresh_requested=true;}
    }
    pub fn set_enabled(&mut self, id: &str, enabled: bool) {
        let job = self.jobs.iter_mut().find(|job|job.id==id).unwrap();
        job.enabled = enabled;
        if !enabled { job.refresh_requested = false; }
    }
    // One inventory lane and one log lane bound load. Repeated manual clicks never queue duplicates.
    pub fn reserve(&mut self, events: bool, manual: Option<&str>) -> Result<Vec<String>> {
        if let Some(id) = manual.filter(|id| *id != "*") {
            if !self.jobs.iter().any(|j| j.id == id && j.events == events) {
                bail!("未知插件");
            }
        }
        if self.busy[usize::from(events)] {
            if manual.is_some() {
                bail!("同类采集正在执行，请完成后重试");
            }
            return Ok(vec![]);
        }
        let now = Instant::now();
        if let Some(id)=manual.filter(|id|*id!="*") {
            if self.jobs.iter().any(|j|j.id==id && j.manual_after>now) {bail!("采集冷却中，请稍后重试；重复点击不会排队");}
        }
        let mut ids = Vec::new();
        for j in &mut self.jobs {
            if j.events != events {
                continue;
            }
            let take = match manual {
                Some("*") => j.enabled && j.manual_after<=now,
                Some(id) => j.id == id,
                None => j.enabled && j.due_at()<=now,
            };
            if take {
                j.phase = "queued";
                j.manual = manual.is_some();
                ids.push(j.id.clone());
            }
        }
        if !ids.is_empty() {
            self.busy[usize::from(events)] = true;
        }
        Ok(ids)
    }
    pub fn begin(&mut self, id: &str) -> bool {
        let j = self.jobs.iter_mut().find(|j| j.id == id).unwrap();
        if !j.manual && (!j.enabled || j.due_at()>Instant::now()) {
            j.phase = "idle";
            return false;
        }
        j.phase = "running";
        j.refresh_requested=false;
        j.started = Some(chrono::Utc::now().to_rfc3339());
        j.began = Some(Instant::now());
        true
    }
    pub fn finish(&mut self, id: &str, mut result: Value) {
        let j = self.jobs.iter_mut().find(|j| j.id == id).unwrap();
        result["duration_ms"] = json!(j.began.take().map(|t| t.elapsed().as_millis()));
        j.result = result;
        j.finished = Some(chrono::Utc::now().to_rfc3339());
        j.phase = "idle";
        let some_log_readable=j.id=="eventlog" && j.result["channels"].as_array().is_some_and(|rows|rows.iter().any(|row|row["ok"]==true));
        j.failures=if j.result["ok"]==false && !some_log_readable {j.failures.saturating_add(1)}else{0};
        let delay=if j.failures>0 {j.seconds.max(30).saturating_mul(1u64<<j.failures.saturating_sub(1).min(6)).min(1800).max(j.seconds)}else{j.seconds};
        j.next = Instant::now() + Duration::from_secs(delay);
        j.manual_after=Instant::now()+Duration::from_secs(if j.events{10}else{30});
        j.result["retry_delay_seconds"]=json!(delay);
        j.result["consecutive_failures"]=json!(j.failures);
    }
    pub fn cancel(&mut self, ids: &[String]) {
        for j in &mut self.jobs {
            if ids.contains(&j.id) {
                j.phase = "idle";
                self.busy[usize::from(j.events)] = false;
            }
        }
    }
    pub fn view(&self) -> Value {
        json!(self.jobs.iter().map(|j|json!({"id":j.id,"enabled":j.enabled,"seconds":j.seconds,"minimum_seconds":if j.events{10}else{30},"phase":j.phase,"last_started":j.started,"last_finished":j.finished,"last_result":j.result,"next_at":if j.enabled&&j.phase=="idle"{Some((chrono::Utc::now()+chrono::Duration::from_std(j.due_at().saturating_duration_since(Instant::now())).unwrap_or_default()).to_rfc3339())}else{None}})).collect::<Vec<_>>())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn global_and_individual_periods_follow_save_order_and_survive_restart() {
        let ids=vec!["scoped".into(),"network".into()];
        let mut schedule=Schedule::new(ids.clone(),&json!({})).unwrap();
        assert_eq!(schedule.global_view()["seconds"],Value::Null);
        schedule.configure("network",false,900);
        let config=schedule.global_candidate(60).unwrap();
        assert!(!config["network"]["enabled"].as_bool().unwrap());
        // Preparing a write must not change live scheduling before persistence succeeds.
        assert_eq!(schedule.jobs[1].seconds,900);
        schedule.configure_global(60);
        assert_eq!(schedule.config(),config);
        assert!(schedule.jobs.iter().all(|job|job.seconds==60));
        assert!(schedule.reserve(false,None).unwrap().is_empty());
        assert!(schedule.reserve(true,None).unwrap().is_empty());
        schedule.configure("scoped",true,120);
        schedule.set_enabled("network",true);
        assert_eq!(schedule.jobs[1].seconds,60);
        let restored=Schedule::new(ids,&schedule.config()).unwrap();
        assert_eq!(restored.jobs[0].seconds,120);
        assert_eq!(restored.runtime_seconds,Some(120));
        assert_eq!(restored.global_view()["seconds"],60);
        let added=Schedule::new(vec!["new".into()],&schedule.config()).unwrap();
        assert_eq!(added.jobs[0].seconds,60);
        // Reapplying the same global value deliberately replaces later individual changes.
        schedule.configure_global(60);
        assert!(schedule.jobs.iter().all(|job|job.seconds==60));
        assert_eq!(schedule.runtime_seconds,Some(60));
    }
    #[test]
    fn global_validation_is_atomic_and_running_work_keeps_latest_period() {
        let mut schedule=Schedule::new(vec!["scoped".into()],&json!({})).unwrap();
        let original=schedule.config();
        for seconds in [0,10,29,604801,u64::MAX] { assert!(schedule.global_candidate(seconds).is_err()); }
        assert_eq!(original,schedule.config());
        for seconds in [30,604800] { assert!(schedule.global_candidate(seconds).is_ok()); }
        let reserved=schedule.reserve(false,None).unwrap();
        assert!(schedule.begin("scoped"));
        schedule.configure_global(60);
        schedule.configure("scoped",true,120);
        assert_eq!(schedule.view()[0]["phase"],"running");
        assert!(schedule.reserve(false,None).unwrap().is_empty());
        schedule.finish("scoped",json!({"ok":true}));
        schedule.cancel(&reserved);
        assert_eq!(schedule.view()[0]["last_result"]["retry_delay_seconds"],120);
        assert!(schedule.reserve(false,None).unwrap().is_empty());
        schedule.configure("eventlog",true,10);
        assert_eq!(schedule.jobs[1].seconds,10);
        assert_eq!(schedule.global_view()["seconds"],60);
    }
    #[test]
    fn changing_period_defers_queued_automatic_work_but_not_an_explicit_run() {
        let mut schedule=Schedule::new(vec!["scoped".into(),"network".into()],&json!({})).unwrap();
        let ids=schedule.reserve(false,None).unwrap();
        assert!(schedule.begin("scoped"));
        schedule.configure_global(300);
        assert!(!schedule.begin("network"));
        schedule.finish("scoped",json!({"ok":true}));schedule.cancel(&ids);
        assert!(schedule.reserve(false,None).unwrap().is_empty());
        schedule.reserve(false,Some("network")).unwrap();
        schedule.configure_global(600);
        assert!(schedule.begin("network"));
    }
    #[test]
    fn device_refresh_is_retained_while_busy_and_respects_cooldown_backoff_and_disabled_jobs() {
        let mut s=Schedule::new(vec!["devices".into()],&json!({})).unwrap();
        let ids=s.reserve(false,None).unwrap();assert!(s.begin("devices"));
        for _ in 0..10000 {s.request_refresh("devices");}
        assert!(s.reserve(false,None).unwrap().is_empty());
        s.finish("devices",json!({"ok":true}));s.cancel(&ids);
        assert!(s.reserve(false,None).unwrap().is_empty());
        s.jobs[0].manual_after=Instant::now();assert_eq!(s.reserve(false,None).unwrap(),vec!["devices"]);
        assert!(s.begin("devices"));assert!(!s.jobs[0].refresh_requested);
        s.request_refresh("devices");s.finish("devices",json!({"ok":false}));s.cancel(&ids);
        s.jobs[0].manual_after=Instant::now();assert!(s.reserve(false,None).unwrap().is_empty());
        s.jobs[0].next=Instant::now();assert_eq!(s.reserve(false,None).unwrap(),vec!["devices"]);
        s.begin("devices");s.finish("devices",json!({"ok":true}));s.cancel(&ids);
        s.configure("devices",false,300);s.request_refresh("devices");assert!(!s.jobs[0].refresh_requested);
        assert!(s.reserve(false,None).unwrap().is_empty());
        s.request_refresh("eventlog");assert!(!s.jobs[1].refresh_requested);
    }
    #[test]
    fn independent_controls_and_manual_disabled_run() {
        let mut s = Schedule::new(vec!["a".into(), "b".into()], &json!({})).unwrap();
        s.configure("a", false, 60);
        assert_eq!(s.reserve(false, None).unwrap(), vec!["b"]);
        assert!(s.reserve(false, Some("b")).is_err());
        assert!(s.begin("b"));
        s.finish("b", json!({"ok":true}));
        assert!(s.reserve(false, Some("b")).is_err());
        s.cancel(&["b".into()]);
        assert!(s.reserve(false, None).unwrap().is_empty());
        assert_eq!(s.reserve(false, Some("a")).unwrap(), vec!["a"]);
        assert!(s.begin("a"));
        s.finish("a", json!({"ok":true}));
        s.cancel(&["a".into()]);
        assert!(!s.jobs[0].enabled);
        let restored = Schedule::new(vec!["a".into(), "b".into()], &s.config()).unwrap();
        assert!(!restored.jobs[0].enabled);
        assert_eq!(restored.jobs[0].seconds, 60);
    }
    #[test]
    fn queued_disable_validation_and_lanes() {
        let mut s = Schedule::new(vec!["a".into()], &json!({})).unwrap();
        assert!(s.candidate("a", true, 0).is_err());
        assert!(s.candidate("bad", true, 300).is_err());
        assert!(s.candidate("usn", true, 9).is_err());
        s.reserve(false, None).unwrap();
        s.configure("a", false, 30);
        assert!(!s.begin("a"));
        assert_eq!(s.reserve(true, Some("usn")).unwrap(), vec!["usn"]);
        assert!(s.begin("usn"));
        s.finish("usn", json!({"ok":false}));
        assert_eq!(s.view()[2]["last_result"]["ok"], false);
    }
    #[test]
    fn failure_backoff_and_manual_clicks_cannot_relaunch_a_hot_loop() {
        let mut s=Schedule::new(vec!["a".into()],&json!({})).unwrap();
        for expected in [30,60,120,240,480,960,1800,1800] {
            s.jobs[1].manual_after=Instant::now();
            assert_eq!(s.reserve(true,Some("eventlog")).unwrap(),vec!["eventlog"]);
            assert!(s.begin("eventlog"));s.finish("eventlog",json!({"ok":false}));s.cancel(&["eventlog".into()]);
            assert_eq!(s.jobs[1].result["retry_delay_seconds"],expected);
            assert!(s.reserve(true,Some("eventlog")).is_err());
            let automatic=s.reserve(true,None).unwrap();assert!(!automatic.contains(&"eventlog".to_string()));s.cancel(&automatic);
        }
        s.jobs[1].manual_after=Instant::now();s.jobs[2].enabled=false;
        s.reserve(true,Some("eventlog")).unwrap();s.begin("eventlog");s.finish("eventlog",json!({"ok":true}));s.cancel(&["eventlog".into()]);
        assert_eq!(s.jobs[1].failures,0);assert_eq!(s.jobs[1].result["retry_delay_seconds"],10);
        assert!(s.reserve(true,Some("*")).unwrap().is_empty());
        // Independent inventory lane remains available; source events never drive this clock.
        assert_eq!(s.reserve(false,None).unwrap(),vec!["a"]);
    }
}
