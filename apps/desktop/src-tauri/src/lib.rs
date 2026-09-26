//! Deepify's platform-independent session backend.
//!
//! The Tauri command layer stays thin. Phase 3 uses a production browser
//! adapter; application and Do Not Disturb ports remain explicit Phase 4 mocks.

pub mod domain {
    use std::fmt;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum SessionState {
        Starting,
        Working,
        Paused,
        Ending,
        Finished,
        Interrupted,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum FinishReason {
        Completed,
        EndedEarly,
        Interrupted,
        ExtensionOrAppCrash,
    }

    impl FinishReason {
        pub fn as_str(self) -> &'static str {
            match self {
                Self::Completed => "completed",
                Self::EndedEarly => "ended_early",
                Self::Interrupted => "interrupted",
                Self::ExtensionOrAppCrash => "extension_or_app_crash",
            }
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct Session {
        pub id: String,
        pub state: SessionState,
        pub planned: Duration,
        pub focused: Duration,
        pub paused: Duration,
        pub intention: Option<String>,
        pub blocked_attempts: u32,
        pub finish_reason: Option<FinishReason>,
        pub started_at: u64,
        pub updated_at: u64,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct TimerSnapshot {
        pub state: SessionState,
        pub remaining: Duration,
        pub focused: Duration,
        pub paused: Duration,
        pub restrictions_active: bool,
        pub blocked_attempts: u32,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct SessionSummary {
        pub id: String,
        pub focused: Duration,
        pub paused: Duration,
        pub blocked_attempts: u32,
        pub reason: FinishReason,
        pub cleanup_complete: bool,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum WhitelistEntry {
        Application(String),
        Website { host: String, path: String },
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct ValidationError {
        pub field: &'static str,
        pub message: String,
    }

    impl fmt::Display for ValidationError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}: {}", self.field, self.message)
        }
    }
    impl std::error::Error for ValidationError {}

    pub fn now_seconds() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    pub fn validate_duration(seconds: u64) -> Result<Duration, ValidationError> {
        if seconds == 0 {
            return Err(ValidationError {
                field: "duration",
                message: "Duration must be greater than zero".into(),
            });
        }
        if seconds > 24 * 60 * 60 {
            return Err(ValidationError {
                field: "duration",
                message: "Duration cannot exceed 24 hours".into(),
            });
        }
        Ok(Duration::from_secs(seconds))
    }

    pub fn normalize_app_id(value: &str) -> Result<String, ValidationError> {
        let value = value.trim();
        if value.is_empty() {
            return Err(ValidationError {
                field: "application",
                message: "App ID is required".into(),
            });
        }
        if value.chars().any(|c| c.is_control() || c.is_whitespace()) {
            return Err(ValidationError {
                field: "application",
                message: "App ID cannot contain whitespace".into(),
            });
        }
        Ok(value.to_string())
    }

    /// Normalize a host/path rule without recording a URL or query string.
    pub fn normalize_website(value: &str) -> Result<(String, String), ValidationError> {
        let raw = value.trim();
        if raw.is_empty() || raw.contains(['?', '#', '@']) {
            return Err(ValidationError {
                field: "website",
                message: "Enter a host and optional path, without query or credentials".into(),
            });
        }
        if raw.contains("://") && !raw.starts_with("http://") && !raw.starts_with("https://") {
            return Err(ValidationError {
                field: "website",
                message: "Only HTTP(S) website rules are supported".into(),
            });
        }
        let candidate = if raw.starts_with("http://") || raw.starts_with("https://") {
            raw.to_string()
        } else if raw.parse::<std::net::Ipv6Addr>().is_ok() {
            format!("http://[{raw}]")
        } else {
            format!("http://{raw}")
        };
        let parsed = url::Url::parse(&candidate).map_err(|_| ValidationError {
            field: "website",
            message: "Enter a valid host name or IP address".into(),
        })?;
        let host = parsed
            .host_str()
            .unwrap_or_default()
            .trim_start_matches('[')
            .trim_end_matches(']')
            .trim_end_matches('.')
            .to_ascii_lowercase();
        if host.is_empty() || !valid_host(&host) {
            return Err(ValidationError {
                field: "website",
                message: "Enter a valid host name or IP address".into(),
            });
        }
        Ok((host, parsed.path().to_string()))
    }

    fn valid_host(host: &str) -> bool {
        if host.parse::<std::net::IpAddr>().is_ok() {
            return true;
        }
        host.len() <= 253
            && host.split('.').all(|label| {
                !label.is_empty()
                    && label.len() <= 63
                    && !label.starts_with('-')
                    && !label.ends_with('-')
                    && label
                        .chars()
                        .all(|character| character.is_ascii_alphanumeric() || character == '-')
            })
    }

    pub fn website_allowed(url: &str, rules: &[WhitelistEntry]) -> bool {
        let Ok(parsed) = url::Url::parse(url) else {
            return !url.to_ascii_lowercase().starts_with("http");
        };
        if parsed.scheme() != "http" && parsed.scheme() != "https" {
            return true;
        }
        let Some(host) = parsed.host_str().map(|host| {
            host.trim_start_matches('[')
                .trim_end_matches(']')
                .to_ascii_lowercase()
        }) else {
            return false;
        };
        if host == "localhost" || host.parse::<std::net::IpAddr>().is_ok_and(is_local_ip) {
            return true;
        }
        rules
            .iter()
            .filter_map(|r| {
                if let WhitelistEntry::Website { host: h, path: p } = r {
                    Some((h, p))
                } else {
                    None
                }
            })
            .any(|(rule_host, rule_path)| {
                (host == *rule_host || host.ends_with(&format!(".{rule_host}")))
                    && parsed.path().starts_with(rule_path)
            })
    }

    fn is_local_ip(ip: std::net::IpAddr) -> bool {
        match ip {
            std::net::IpAddr::V4(address) => {
                address.is_private() || address.is_loopback() || address.is_link_local()
            }
            std::net::IpAddr::V6(address) => {
                address.is_loopback()
                    || address.is_unique_local()
                    || address.is_unicast_link_local()
                    || address.to_ipv4_mapped().is_some_and(is_local_ip_v4)
            }
        }
    }

    fn is_local_ip_v4(address: std::net::Ipv4Addr) -> bool {
        address.is_private() || address.is_loopback() || address.is_link_local()
    }

    pub fn implicit_app_allowed(app_id: Option<&str>, focus_app: &str) -> bool {
        app_id.is_none() || app_id == Some(focus_app)
    }
}

pub mod audio;
pub mod browser;
#[cfg(target_os = "linux")]
pub mod display;
pub mod settings;
pub mod storage;

pub mod services {
    use super::domain::*;
    use crate::browser::BrowserBroker;
    use std::collections::HashMap;
    use std::time::Duration;

    pub trait Clock: Clone {
        fn now(&self) -> u64;
    }
    #[derive(Clone, Default)]
    pub struct SystemClock;
    impl Clock for SystemClock {
        fn now(&self) -> u64 {
            now_seconds()
        }
    }

    #[derive(Clone, Default)]
    pub struct FakeClock(pub std::rc::Rc<std::cell::Cell<u64>>);
    impl FakeClock {
        pub fn new(at: u64) -> Self {
            Self(std::rc::Rc::new(std::cell::Cell::new(at)))
        }
        pub fn advance(&self, seconds: u64) {
            self.0.set(self.now() + seconds);
        }
    }
    impl Clock for FakeClock {
        fn now(&self) -> u64 {
            self.0.get()
        }
    }

    pub trait RestrictionAdapter {
        fn preflight(&mut self) -> Result<(), String>;
        fn activate(&mut self) -> Result<(), String>;
        fn deactivate(&mut self) -> Result<(), String>;
        fn healthy(&self) -> bool;
    }
    #[derive(Debug, Clone, Copy, Eq, PartialEq)]
    pub enum MockFailure {
        None,
        Preflight,
        Activate,
        Deactivate,
    }
    #[derive(Debug, Clone)]
    pub struct MockRestriction {
        pub failure: MockFailure,
        pub latency: Duration,
        pub active: bool,
        pub cleanup_attempts: u32,
        pub blocked_attempts: u32,
    }
    impl Default for MockRestriction {
        fn default() -> Self {
            Self {
                failure: MockFailure::None,
                latency: Duration::ZERO,
                active: false,
                cleanup_attempts: 0,
                blocked_attempts: 0,
            }
        }
    }
    impl RestrictionAdapter for MockRestriction {
        fn preflight(&mut self) -> Result<(), String> {
            std::thread::sleep(self.latency);
            if self.failure == MockFailure::Preflight {
                Err("simulated integration unavailable".into())
            } else {
                Ok(())
            }
        }
        fn activate(&mut self) -> Result<(), String> {
            std::thread::sleep(self.latency);
            if self.failure == MockFailure::Activate {
                Err("simulated activation failure".into())
            } else {
                self.active = true;
                Ok(())
            }
        }
        fn deactivate(&mut self) -> Result<(), String> {
            std::thread::sleep(self.latency);
            self.cleanup_attempts += 1;
            if self.failure == MockFailure::Deactivate {
                Err("simulated cleanup failure".into())
            } else {
                self.active = false;
                Ok(())
            }
        }
        fn healthy(&self) -> bool {
            self.failure == MockFailure::None
        }
    }

    pub trait RestrictionStep {
        fn component(&self) -> &'static str;
        fn preflight(&mut self) -> Result<(), String>;
        fn activate(&mut self) -> Result<(), String>;
        fn deactivate(&mut self) -> Result<(), String>;
        fn healthy(&self) -> bool;
    }

    pub trait BrowserRestrictionAdapter: RestrictionStep {}
    pub trait ApplicationRestrictionAdapter: RestrictionStep {}
    pub trait DoNotDisturbAdapter: RestrictionStep {}

    #[derive(Debug, Clone)]
    pub struct MockRestrictionStep {
        pub component: &'static str,
        pub failure: MockFailure,
        pub latency: Duration,
        pub active: bool,
        pub cleanup_attempts: u32,
    }

    impl MockRestrictionStep {
        pub fn healthy(component: &'static str) -> Self {
            Self {
                component,
                failure: MockFailure::None,
                latency: Duration::ZERO,
                active: false,
                cleanup_attempts: 0,
            }
        }
    }

    impl RestrictionStep for MockRestrictionStep {
        fn component(&self) -> &'static str {
            self.component
        }
        fn preflight(&mut self) -> Result<(), String> {
            std::thread::sleep(self.latency);
            if self.failure == MockFailure::Preflight {
                Err(format!("{} preflight failed (simulated)", self.component))
            } else {
                Ok(())
            }
        }
        fn activate(&mut self) -> Result<(), String> {
            std::thread::sleep(self.latency);
            if self.failure == MockFailure::Activate {
                Err(format!("{} activation failed (simulated)", self.component))
            } else {
                self.active = true;
                Ok(())
            }
        }
        fn deactivate(&mut self) -> Result<(), String> {
            std::thread::sleep(self.latency);
            self.cleanup_attempts += 1;
            if self.failure == MockFailure::Deactivate {
                Err(format!("{} cleanup failed (simulated)", self.component))
            } else {
                self.active = false;
                Ok(())
            }
        }
        fn healthy(&self) -> bool {
            self.failure == MockFailure::None
        }
    }

    impl BrowserRestrictionAdapter for MockRestrictionStep {}
    impl ApplicationRestrictionAdapter for MockRestrictionStep {}
    impl DoNotDisturbAdapter for MockRestrictionStep {}

    /// The only non-mock Phase 3 restriction step. Policy is supplied by the
    /// command layer immediately before a session starts; the native host never
    /// receives this policy except as a typed browser protocol message.
    #[derive(Clone)]
    struct BrowserStartPolicy {
        session_id: String,
        rules: Vec<(String, String)>,
        seconds: u64,
    }
    #[derive(Clone)]
    pub struct ProductionBrowserRestrictionStep {
        broker: BrowserBroker,
        pending: Option<BrowserStartPolicy>,
        active_session: Option<String>,
    }
    impl ProductionBrowserRestrictionStep {
        pub fn new(broker: BrowserBroker) -> Self {
            Self {
                broker,
                pending: None,
                active_session: None,
            }
        }
        pub fn configure(
            &mut self,
            session_id: String,
            rules: Vec<(String, String)>,
            seconds: u64,
        ) {
            self.pending = Some(BrowserStartPolicy {
                session_id,
                rules,
                seconds,
            });
        }
    }
    impl RestrictionStep for ProductionBrowserRestrictionStep {
        fn component(&self) -> &'static str {
            "browser"
        }
        fn preflight(&mut self) -> Result<(), String> {
            self.broker
                .check_health()
                .map_err(|error| format!("browser preflight failed: {error}"))
        }
        fn activate(&mut self) -> Result<(), String> {
            let policy = self
                .pending
                .take()
                .ok_or_else(|| "browser policy was not prepared".to_string())?;
            self.broker
                .start_session(&policy.session_id, policy.rules, "working", policy.seconds)
                .map_err(|error| format!("browser activation failed: {error}"))?;
            self.active_session = Some(policy.session_id);
            Ok(())
        }
        fn deactivate(&mut self) -> Result<(), String> {
            if let Some(session_id) = self.active_session.as_deref() {
                self.broker
                    .stop_session(session_id)
                    .map_err(|error| format!("browser cleanup failed: {error}"))?;
                self.active_session = None;
            }
            Ok(())
        }
        fn healthy(&self) -> bool {
            matches!(self.broker.snapshot().state, "healthy_idle" | "active")
        }
    }
    impl BrowserRestrictionAdapter for ProductionBrowserRestrictionStep {}

    #[derive(Debug, Clone)]
    pub struct RestrictionCoordinator<B, A, D> {
        pub browser: B,
        pub applications: A,
        pub do_not_disturb: D,
    }

    pub type MockRestrictionCoordinator =
        RestrictionCoordinator<MockRestrictionStep, MockRestrictionStep, MockRestrictionStep>;
    pub type ProductionRestrictionCoordinator = RestrictionCoordinator<
        ProductionBrowserRestrictionStep,
        MockRestrictionStep,
        MockRestrictionStep,
    >;

    impl ProductionRestrictionCoordinator {
        pub fn new(browser: BrowserBroker) -> Self {
            Self {
                browser: ProductionBrowserRestrictionStep::new(browser),
                applications: MockRestrictionStep::healthy("applications"),
                do_not_disturb: MockRestrictionStep::healthy("do_not_disturb"),
            }
        }
    }

    impl Default for MockRestrictionCoordinator {
        fn default() -> Self {
            Self {
                browser: MockRestrictionStep::healthy("browser"),
                applications: MockRestrictionStep::healthy("applications"),
                do_not_disturb: MockRestrictionStep::healthy("do_not_disturb"),
            }
        }
    }

    impl<B, A, D> RestrictionCoordinator<B, A, D>
    where
        B: BrowserRestrictionAdapter,
        A: ApplicationRestrictionAdapter,
        D: DoNotDisturbAdapter,
    {
        fn cleanup_all(&mut self) -> Result<(), String> {
            let mut failures = Vec::new();
            for result in [
                self.do_not_disturb.deactivate(),
                self.applications.deactivate(),
                self.browser.deactivate(),
            ] {
                if let Err(error) = result {
                    failures.push(error);
                }
            }
            if failures.is_empty() {
                Ok(())
            } else {
                Err(failures.join("; "))
            }
        }
    }

    impl<B, A, D> RestrictionAdapter for RestrictionCoordinator<B, A, D>
    where
        B: BrowserRestrictionAdapter,
        A: ApplicationRestrictionAdapter,
        D: DoNotDisturbAdapter,
    {
        fn preflight(&mut self) -> Result<(), String> {
            self.browser.preflight()?;
            self.applications.preflight()?;
            self.do_not_disturb.preflight()
        }
        fn activate(&mut self) -> Result<(), String> {
            if let Err(error) = self.browser.activate() {
                let _ = self.cleanup_all();
                return Err(error);
            }
            if let Err(error) = self.applications.activate() {
                let _ = self.cleanup_all();
                return Err(error);
            }
            if let Err(error) = self.do_not_disturb.activate() {
                let _ = self.cleanup_all();
                return Err(error);
            }
            Ok(())
        }
        fn deactivate(&mut self) -> Result<(), String> {
            self.cleanup_all()
        }
        fn healthy(&self) -> bool {
            self.browser.healthy() && self.applications.healthy() && self.do_not_disturb.healthy()
        }
    }

    #[derive(Debug, Clone, Eq, PartialEq)]
    pub enum SessionError {
        Invalid(String),
        Conflict,
        IllegalTransition,
        Integration(String),
        CleanupIncomplete,
    }
    impl std::fmt::Display for SessionError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{self:?}")
        }
    }
    impl std::error::Error for SessionError {}

    pub struct SessionService<C: Clock, R: RestrictionAdapter> {
        pub clock: C,
        pub restriction: R,
        pub current: Option<Session>,
        pub history: Vec<SessionSummary>,
        pub recovery_required: bool,
        sequence: u64,
    }
    impl<C: Clock, R: RestrictionAdapter> SessionService<C, R> {
        pub fn new(clock: C, restriction: R) -> Self {
            Self {
                clock,
                restriction,
                current: None,
                history: Vec::new(),
                recovery_required: false,
                sequence: 0,
            }
        }
        pub fn start(
            &mut self,
            seconds: u64,
            intention: Option<String>,
        ) -> Result<TimerSnapshot, SessionError> {
            if self.current.is_some() || self.recovery_required {
                return Err(SessionError::Conflict);
            }
            let planned =
                validate_duration(seconds).map_err(|e| SessionError::Invalid(e.to_string()))?;
            if let Err(error) = self.restriction.preflight() {
                self.recovery_required = self.restriction.deactivate().is_err();
                return Err(SessionError::Integration(error));
            }
            self.sequence += 1;
            let mut session = Session {
                id: format!("session-{}", self.sequence),
                state: SessionState::Starting,
                planned,
                focused: Duration::ZERO,
                paused: Duration::ZERO,
                intention,
                blocked_attempts: 0,
                finish_reason: None,
                started_at: self.clock.now(),
                updated_at: self.clock.now(),
            };
            if let Err(error) = self.restriction.activate() {
                self.recovery_required = self.restriction.deactivate().is_err();
                return Err(SessionError::Integration(error));
            }
            session.state = SessionState::Working;
            self.current = Some(session);
            Ok(self.snapshot())
        }
        pub fn snapshot(&self) -> TimerSnapshot {
            let s = self
                .current
                .as_ref()
                .expect("snapshot requires active session");
            TimerSnapshot {
                state: s.state,
                remaining: s.planned.saturating_sub(s.focused),
                focused: s.focused,
                paused: s.paused,
                restrictions_active: matches!(
                    s.state,
                    SessionState::Working | SessionState::Paused | SessionState::Ending
                ),
                blocked_attempts: s.blocked_attempts,
            }
        }
        fn accrue(&mut self) {
            if let Some(s) = self.current.as_mut() {
                let elapsed = self.clock.now().saturating_sub(s.updated_at);
                match s.state {
                    SessionState::Working => s.focused += Duration::from_secs(elapsed),
                    SessionState::Paused => s.paused += Duration::from_secs(elapsed),
                    _ => {}
                }
                s.updated_at = self.clock.now();
            }
        }
        pub fn pause(&mut self) -> Result<TimerSnapshot, SessionError> {
            self.accrue();
            let s = self
                .current
                .as_mut()
                .ok_or(SessionError::IllegalTransition)?;
            if s.state != SessionState::Working {
                return Err(SessionError::IllegalTransition);
            }
            s.state = SessionState::Paused;
            Ok(self.snapshot())
        }
        pub fn resume(&mut self) -> Result<TimerSnapshot, SessionError> {
            self.accrue();
            let s = self
                .current
                .as_mut()
                .ok_or(SessionError::IllegalTransition)?;
            if s.state != SessionState::Paused {
                return Err(SessionError::IllegalTransition);
            }
            s.updated_at = self.clock.now();
            s.state = SessionState::Working;
            Ok(self.snapshot())
        }
        pub fn blocked_attempt(&mut self) {
            if let Some(s) = self.current.as_mut() {
                s.blocked_attempts += 1;
            }
        }
        pub fn finish(&mut self, reason: FinishReason) -> Result<SessionSummary, SessionError> {
            self.accrue();
            let mut s = self.current.take().ok_or(SessionError::IllegalTransition)?;
            s.state = SessionState::Ending;
            let cleanup = self.restriction.deactivate().is_ok();
            self.recovery_required = !cleanup;
            s.state = if matches!(
                reason,
                FinishReason::Interrupted | FinishReason::ExtensionOrAppCrash
            ) {
                SessionState::Interrupted
            } else {
                SessionState::Finished
            };
            s.finish_reason = Some(reason);
            let summary = SessionSummary {
                id: s.id,
                focused: s.focused,
                paused: s.paused,
                blocked_attempts: s.blocked_attempts,
                reason,
                cleanup_complete: cleanup,
            };
            self.history.push(summary.clone());
            Ok(summary)
        }
        pub fn tick(&mut self) -> Result<Option<SessionSummary>, SessionError> {
            self.accrue();
            if self
                .current
                .as_ref()
                .is_some_and(|s| s.focused >= s.planned)
            {
                Ok(Some(self.finish(FinishReason::Completed)?))
            } else {
                Ok(None)
            }
        }
        pub fn recover_abandoned(&mut self) -> Option<SessionSummary> {
            self.current.take().map(|s| {
                let cleanup_complete = self.restriction.deactivate().is_ok();
                self.recovery_required = !cleanup_complete;
                let summary = SessionSummary {
                    id: s.id,
                    focused: s.focused,
                    paused: s.paused,
                    blocked_attempts: s.blocked_attempts,
                    reason: FinishReason::Interrupted,
                    cleanup_complete,
                };
                self.history.push(summary.clone());
                summary
            })
        }
        pub fn runtime_failure(
            &mut self,
            _component: &str,
        ) -> Result<SessionSummary, SessionError> {
            self.finish(FinishReason::ExtensionOrAppCrash)
        }
        pub fn retry_cleanup(&mut self) -> Result<(), SessionError> {
            if !self.recovery_required {
                return Ok(());
            }
            self.restriction
                .deactivate()
                .map_err(|_| SessionError::CleanupIncomplete)?;
            self.recovery_required = false;
            Ok(())
        }
    }

    #[derive(Default)]
    pub struct WhitelistService {
        entries: HashMap<String, WhitelistEntry>,
    }
    impl WhitelistService {
        pub fn add(
            &mut self,
            value: &str,
            website: bool,
        ) -> Result<WhitelistEntry, ValidationError> {
            let entry = if website {
                let (host, path) = normalize_website(value)?;
                WhitelistEntry::Website { host, path }
            } else {
                WhitelistEntry::Application(normalize_app_id(value)?)
            };
            let key = format!("{entry:?}");
            self.entries.insert(key, entry.clone());
            Ok(entry)
        }
        pub fn remove(&mut self, key: &str, active: bool) -> Result<(), SessionError> {
            if active {
                return Err(SessionError::Conflict);
            }
            self.entries.remove(key);
            Ok(())
        }
        pub fn all(&self) -> Vec<WhitelistEntry> {
            self.entries.values().cloned().collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::domain::*;
    use super::services::*;
    use std::time::Duration;
    #[test]
    fn duration_is_required() {
        assert!(validate_duration(0).is_err());
        assert_eq!(validate_duration(60).unwrap(), Duration::from_secs(60));
    }
    #[test]
    fn pause_freezes_focus_and_keeps_restrictions() {
        let c = FakeClock::new(10);
        let mut s = SessionService::new(c.clone(), MockRestriction::default());
        s.start(100, None).unwrap();
        c.advance(20);
        let p = s.pause().unwrap();
        assert_eq!(p.focused, Duration::from_secs(20));
        assert!(p.restrictions_active);
        c.advance(20);
        assert_eq!(s.resume().unwrap().focused, Duration::from_secs(20));
    }
    #[test]
    fn completion_tracks_actual_time() {
        let c = FakeClock::new(1);
        let mut s = SessionService::new(c.clone(), MockRestriction::default());
        s.start(10, None).unwrap();
        c.advance(10);
        let out = s.tick().unwrap().unwrap();
        assert_eq!(out.reason, FinishReason::Completed);
        assert_eq!(out.focused, Duration::from_secs(10));
    }
    #[test]
    fn early_end_needs_no_reason() {
        let mut s = SessionService::new(FakeClock::new(1), MockRestriction::default());
        s.start(100, None).unwrap();
        assert_eq!(
            s.finish(FinishReason::EndedEarly).unwrap().reason,
            FinishReason::EndedEarly
        );
    }
    #[test]
    fn duplicate_start_conflicts() {
        let mut s = SessionService::new(FakeClock::new(1), MockRestriction::default());
        s.start(1, None).unwrap();
        assert_eq!(s.start(1, None), Err(SessionError::Conflict));
    }
    #[test]
    fn activation_failure_cleans_up() {
        let r = MockRestriction {
            failure: MockFailure::Activate,
            ..Default::default()
        };
        let mut s = SessionService::new(FakeClock::new(1), r);
        assert!(matches!(
            s.start(1, None),
            Err(SessionError::Integration(_))
        ));
        assert_eq!(s.restriction.cleanup_attempts, 1);
        assert!(s.current.is_none());
    }
    #[test]
    fn recovery_interrupts() {
        let c = FakeClock::new(1);
        let mut s = SessionService::new(c, MockRestriction::default());
        s.start(5, None).unwrap();
        let x = s.recover_abandoned().unwrap();
        assert_eq!(x.reason, FinishReason::Interrupted);
        assert!(s.current.is_none());
    }
    #[test]
    fn cleanup_failure_is_reported_and_does_not_claim_success() {
        let restriction = MockRestriction {
            failure: MockFailure::Deactivate,
            ..Default::default()
        };
        let mut service = SessionService::new(FakeClock::new(1), restriction);
        service.start(10, None).unwrap();
        let summary = service.finish(FinishReason::EndedEarly).unwrap();
        assert!(!summary.cleanup_complete);
        assert_eq!(service.restriction.cleanup_attempts, 1);
    }
    #[test]
    fn coordinator_cleans_up_every_startup_failure() {
        for failure in [MockFailure::Preflight, MockFailure::Activate] {
            for component in 0..3 {
                let mut restriction = MockRestrictionCoordinator::default();
                match component {
                    0 => restriction.browser.failure = failure,
                    1 => restriction.applications.failure = failure,
                    _ => restriction.do_not_disturb.failure = failure,
                }
                let mut service = SessionService::new(FakeClock::new(1), restriction);
                assert!(matches!(
                    service.start(10, None),
                    Err(SessionError::Integration(_))
                ));
                assert!(service.current.is_none());
                assert!(service.restriction.browser.cleanup_attempts >= 1);
                assert!(service.restriction.applications.cleanup_attempts >= 1);
                assert!(service.restriction.do_not_disturb.cleanup_attempts >= 1);
            }
        }
    }
    #[test]
    fn coordinator_reports_every_cleanup_failure_and_requires_recovery() {
        for component in 0..3 {
            let mut service =
                SessionService::new(FakeClock::new(1), MockRestrictionCoordinator::default());
            service.start(10, None).unwrap();
            match component {
                0 => service.restriction.browser.failure = MockFailure::Deactivate,
                1 => service.restriction.applications.failure = MockFailure::Deactivate,
                _ => service.restriction.do_not_disturb.failure = MockFailure::Deactivate,
            }
            let summary = service.finish(FinishReason::EndedEarly).unwrap();
            assert!(!summary.cleanup_complete);
            assert!(service.recovery_required);
            assert_eq!(service.start(10, None), Err(SessionError::Conflict));
            service.restriction.browser.failure = MockFailure::None;
            service.restriction.applications.failure = MockFailure::None;
            service.restriction.do_not_disturb.failure = MockFailure::None;
            service.retry_cleanup().unwrap();
            assert!(!service.recovery_required);
        }
    }
    #[test]
    fn configurable_mock_latency_applies_to_each_adapter_step() {
        let mut step = MockRestrictionStep::healthy("browser");
        step.latency = Duration::from_millis(2);
        let started = std::time::Instant::now();
        step.preflight().unwrap();
        step.activate().unwrap();
        step.deactivate().unwrap();
        assert!(started.elapsed() >= Duration::from_millis(6));
    }
    #[test]
    fn malformed_whitelist_is_atomic() {
        let mut w = WhitelistService::default();
        assert!(w.add("bad value", false).is_err());
        assert!(w.all().is_empty());
        assert!(w.add("example.com/docs", true).is_ok());
    }
    #[test]
    fn url_rules_follow_phase_one() {
        let rules = vec![WhitelistEntry::Website {
            host: "example.com".into(),
            path: "/docs".into(),
        }];
        assert!(website_allowed("https://example.com/docs/page:80", &rules));
        assert!(website_allowed("http://docs.example.com/docs", &rules));
        assert!(!website_allowed("https://other.example.net/docs", &rules));
        assert!(website_allowed("ftp://other.example.net", &rules));
        assert!(website_allowed("http://127.0.0.1:8000", &rules));
    }
    #[test]
    fn rust_url_matcher_uses_the_shared_extension_fixture_matrix() {
        #[derive(serde::Deserialize)]
        struct Fixture {
            version: u8,
            cases: Vec<Case>,
        }
        #[derive(serde::Deserialize)]
        struct Case {
            url: String,
            rules: Vec<String>,
            allowed: bool,
        }
        let fixture: Fixture =
            serde_json::from_str(include_str!("../../../../contracts/url-rule-cases.json"))
                .unwrap();
        assert_eq!(fixture.version, 1);
        for case in fixture.cases {
            let rules = case
                .rules
                .iter()
                .map(|rule| normalize_website(rule).unwrap())
                .map(|(host, path)| WhitelistEntry::Website { host, path })
                .collect::<Vec<_>>();
            assert_eq!(
                website_allowed(&case.url, &rules),
                case.allowed,
                "{}",
                case.url
            );
        }
    }
    #[test]
    fn unknown_app_is_allowed_mvp() {
        assert!(implicit_app_allowed(None, "com.deepify.desktop"));
        assert!(implicit_app_allowed(
            Some("com.deepify.desktop"),
            "com.deepify.desktop"
        ));
    }
}
