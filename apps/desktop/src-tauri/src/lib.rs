//! Deepify's platform-independent Phase 2 backend.
//!
//! The Tauri command layer is intentionally thin and can be added without
//! changing these rules. Restriction ports below are mock implementations in
//! this phase; they never claim to block a real app or website.

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
        let without_scheme = raw
            .strip_prefix("http://")
            .or_else(|| raw.strip_prefix("https://"))
            .unwrap_or(raw);
        if without_scheme.contains("://") {
            return Err(ValidationError {
                field: "website",
                message: "Only HTTP(S) website rules are supported".into(),
            });
        }
        let (authority, path) = without_scheme
            .split_once('/')
            .map_or((without_scheme, "/"), |(h, _p)| {
                (h, &without_scheme[h.len()..])
            });
        let host = authority
            .trim_matches(['[', ']'])
            .trim_end_matches('.')
            .to_ascii_lowercase();
        if host.is_empty()
            || host.chars().any(|c| c.is_whitespace() || c == '_')
            || (!is_ip(host.as_str()) && host.contains(':'))
        {
            return Err(ValidationError {
                field: "website",
                message: "Enter a valid host name or IP address".into(),
            });
        }
        let path = if path.is_empty() { "/" } else { path };
        if !path.starts_with('/') {
            return Err(ValidationError {
                field: "website",
                message: "Path must begin with /".into(),
            });
        }
        Ok((host, path.to_string()))
    }

    fn is_ip(host: &str) -> bool {
        host.parse::<std::net::Ipv4Addr>().is_ok() || host.parse::<std::net::Ipv6Addr>().is_ok()
    }

    pub fn website_allowed(url: &str, rules: &[WhitelistEntry]) -> bool {
        let Some((scheme, remainder)) = url.split_once("://") else {
            return true;
        };
        if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
            return true;
        }
        let authority_end = remainder.find('/').unwrap_or(remainder.len());
        let authority = &remainder[..authority_end];
        let path = &remainder[authority_end..];
        let host_port = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
        let host = if host_port.starts_with('[') {
            host_port
                .split(']')
                .next()
                .unwrap_or(host_port)
                .trim_start_matches('[')
                .to_ascii_lowercase()
        } else {
            host_port
                .rsplit_once(':')
                .map_or(host_port, |(h, p)| {
                    if p.parse::<u16>().is_ok() {
                        h
                    } else {
                        host_port
                    }
                })
                .to_ascii_lowercase()
        };
        let local_172 = host
            .strip_prefix("172.")
            .and_then(|n| n.split('.').next())
            .and_then(|n| n.parse::<u8>().ok())
            .is_some_and(|n| (16..=31).contains(&n));
        if host == "localhost"
            || host == "127.0.0.1"
            || host == "::1"
            || host.starts_with("192.168.")
            || host.starts_with("10.")
            || local_172
        {
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
                    && path.starts_with(rule_path)
            })
    }

    pub fn implicit_app_allowed(app_id: Option<&str>, focus_app: &str) -> bool {
        app_id.is_none() || app_id == Some(focus_app)
    }
}

pub mod audio;
pub mod settings;
pub mod storage;

pub mod services {
    use super::domain::*;
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
        pub active: bool,
        pub cleanup_attempts: u32,
        pub blocked_attempts: u32,
    }
    impl Default for MockRestriction {
        fn default() -> Self {
            Self {
                failure: MockFailure::None,
                active: false,
                cleanup_attempts: 0,
                blocked_attempts: 0,
            }
        }
    }
    impl RestrictionAdapter for MockRestriction {
        fn preflight(&mut self) -> Result<(), String> {
            if self.failure == MockFailure::Preflight {
                Err("simulated integration unavailable".into())
            } else {
                Ok(())
            }
        }
        fn activate(&mut self) -> Result<(), String> {
            if self.failure == MockFailure::Activate {
                Err("simulated activation failure".into())
            } else {
                self.active = true;
                Ok(())
            }
        }
        fn deactivate(&mut self) -> Result<(), String> {
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
        sequence: u64,
    }
    impl<C: Clock, R: RestrictionAdapter> SessionService<C, R> {
        pub fn new(clock: C, restriction: R) -> Self {
            Self {
                clock,
                restriction,
                current: None,
                history: Vec::new(),
                sequence: 0,
            }
        }
        pub fn start(
            &mut self,
            seconds: u64,
            intention: Option<String>,
        ) -> Result<TimerSnapshot, SessionError> {
            if self.current.is_some() {
                return Err(SessionError::Conflict);
            }
            let planned =
                validate_duration(seconds).map_err(|e| SessionError::Invalid(e.to_string()))?;
            self.restriction
                .preflight()
                .map_err(SessionError::Integration)?;
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
                let _ = self.restriction.deactivate();
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
            s.state = if reason == FinishReason::Interrupted {
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
            if cleanup {
                Ok(summary)
            } else {
                Err(SessionError::CleanupIncomplete)
            }
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
                let summary = SessionSummary {
                    id: s.id,
                    focused: s.focused,
                    paused: s.paused,
                    blocked_attempts: s.blocked_attempts,
                    reason: FinishReason::Interrupted,
                    cleanup_complete: self.restriction.deactivate().is_ok(),
                };
                self.history.push(summary.clone());
                summary
            })
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
        assert_eq!(
            service.finish(FinishReason::EndedEarly),
            Err(SessionError::CleanupIncomplete)
        );
        assert!(!service.history[0].cleanup_complete);
        assert_eq!(service.restriction.cleanup_attempts, 1);
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
    fn unknown_app_is_allowed_mvp() {
        assert!(implicit_app_allowed(None, "com.deepify.desktop"));
        assert!(implicit_app_allowed(
            Some("com.deepify.desktop"),
            "com.deepify.desktop"
        ));
    }
}
