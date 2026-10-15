use std::{process::Child, time::{Duration, Instant}};

/// All windows and permission switches share one launch gate. The daemon additionally reserves
/// its directory-specific first pipe instance before opening storage or running collectors.
#[derive(Default)]
pub struct LaunchState {
    pub child: Option<Child>,
    blocked_until: Option<Instant>,
    attempts: u8,
    stable_since: Option<Instant>,
}

impl LaunchState {
    pub fn defer(&mut self, now: Instant, seconds: u64) {
        self.blocked_until = Some(now + Duration::from_secs(seconds));
    }

    pub fn allowed(&self, now: Instant) -> bool {
        self.blocked_until.is_none_or(|until| now >= until)
    }

    pub fn connected(&mut self, now: Instant) {
        let since = self.stable_since.get_or_insert(now);
        if now.duration_since(*since) >= Duration::from_secs(120) { self.attempts = 0; }
    }

    pub fn disconnected(&mut self) { self.stable_since = None; }

    pub fn reserve_attempt(&mut self, now: Instant, manual: bool) -> Result<(), String> {
        if !self.allowed(now) {return Err("后台正在恢复或切换权限，请稍候。".into());}
        if manual {self.attempts = 0;}
        if self.attempts >= 3 {return Err("后台多次启动失败，已暂停自动启动。请重新连接。".into());}
        self.attempts += 1;
        self.defer(now, if self.attempts == 1 {10} else {30});
        Ok(())
    }

    pub fn child_running(&mut self) -> Result<bool, String> {
        match self.child.as_mut().map(Child::try_wait).transpose().map_err(|e| e.to_string())? {
            Some(None) => Ok(true),
            _ => { self.child = None; Ok(false) }
        }
    }
}

#[derive(Default)]
pub struct ServiceControl(pub tokio::sync::Mutex<LaunchState>);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn launch_cooldown_and_permission_switch_grace_are_bounded() {
        let now = Instant::now();
        let mut state = LaunchState::default();
        assert!(state.allowed(now));
        state.defer(now, 10);
        assert!(!state.allowed(now + Duration::from_secs(9)));
        assert!(state.allowed(now + Duration::from_secs(10)));
        state.defer(now, 180);
        assert!(!state.allowed(now + Duration::from_secs(179)));
        assert!(state.allowed(now + Duration::from_secs(180)));
        assert!(!state.child_running().unwrap());
    }
    #[test]
    fn concurrent_launch_and_permission_switch_cannot_enter() {
        let control = ServiceControl::default();
        let guard = control.0.try_lock().unwrap();
        assert!(control.0.try_lock().is_err());
        drop(guard);
        assert!(control.0.try_lock().is_ok());
    }

    #[test]
    fn short_lived_restarts_share_a_budget_and_manual_retry_can_resume() {
        let now=Instant::now();let mut state=LaunchState::default();
        for seconds in [0,10,40] {
            let at=now+Duration::from_secs(seconds);
            state.reserve_attempt(at,false).unwrap();state.connected(at);state.disconnected();
        }
        let later=now+Duration::from_secs(1000);
        assert!(state.reserve_attempt(later,false).is_err());
        state.reserve_attempt(later,true).unwrap();
        state.connected(later);state.connected(later+Duration::from_secs(120));state.disconnected();
        state.reserve_attempt(later+Duration::from_secs(120),false).unwrap();
    }
}
