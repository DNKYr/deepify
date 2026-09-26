//! Niri window policy and a single, bounded event-stream connection.
use crate::{
    domain::implicit_app_allowed,
    platform_command::{self, ManagedChild, OUTPUT_LIMIT, TIMEOUT},
    services::{ApplicationRestrictionAdapter, RestrictionStep},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{BufRead, BufReader, Read},
    sync::mpsc::{self, Receiver, TryRecvError},
    thread::JoinHandle,
    time::Instant,
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Window {
    pub id: u64,
    pub app_id: Option<String>,
    pub is_focused: bool,
}

#[derive(Deserialize)]
enum Event {
    WindowsChanged { windows: Vec<Window> },
    WindowOpenedOrChanged { window: Window },
    WindowClosed { id: u64 },
    WindowFocusChanged { id: Option<u64> },
    Other,
}

fn parse_event(input: &[u8]) -> Result<Event, String> {
    let value: serde_json::Value =
        serde_json::from_slice(input).map_err(|_| "Niri event was invalid")?;
    let object = value
        .as_object()
        .filter(|object| object.len() == 1)
        .ok_or("Niri event was invalid")?;
    if object.keys().any(|key| {
        matches!(
            key.as_str(),
            "WindowsChanged" | "WindowOpenedOrChanged" | "WindowClosed" | "WindowFocusChanged"
        )
    }) {
        serde_json::from_value(value).map_err(|_| "Niri window event was invalid".into())
    } else {
        Ok(Event::Other)
    }
}

pub trait NiriCommands: Send {
    fn windows(&mut self) -> Result<Vec<Window>, String>;
    fn close(&mut self, id: u64) -> Result<(), String>;
    fn focus(&mut self, id: u64) -> Result<(), String>;
}

#[derive(Default)]
pub struct NiriIpc;
impl NiriCommands for NiriIpc {
    fn windows(&mut self) -> Result<Vec<Window>, String> {
        serde_json::from_slice(&platform_command::run("niri", &["msg", "-j", "windows"])?)
            .map_err(|_| "Niri returned an invalid window inventory".into())
    }
    fn close(&mut self, id: u64) -> Result<(), String> {
        platform_command::run(
            "niri",
            &["msg", "action", "close-window", "--id", &id.to_string()],
        )
        .map(|_| ())
    }
    fn focus(&mut self, id: u64) -> Result<(), String> {
        platform_command::run(
            "niri",
            &["msg", "action", "focus-window", "--id", &id.to_string()],
        )
        .map(|_| ())
    }
}

#[derive(Default)]
pub struct WindowPolicy {
    pub windows: BTreeMap<u64, Window>,
    allowed: BTreeSet<String>,
    attempted: BTreeSet<u64>,
    last_allowed: Option<u64>,
    blocked_attempts: u32,
    pub latest_notice: Option<String>,
}

impl WindowPolicy {
    pub fn configure(&mut self, allowed: Vec<String>) {
        self.allowed = allowed.into_iter().collect();
        self.attempted.clear();
        self.blocked_attempts = 0;
        self.latest_notice = None;
    }
    pub fn allowed(&self, window: &Window) -> bool {
        implicit_app_allowed(window.app_id.as_deref(), "com.deepify.desktop")
            || window
                .app_id
                .as_ref()
                .is_some_and(|id| self.allowed.contains(id))
    }
    pub fn blocked_apps(&self) -> Vec<String> {
        self.windows
            .values()
            .filter(|window| !self.allowed(window))
            .filter_map(|window| window.app_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
    pub fn unidentified_count(&self) -> usize {
        self.windows
            .values()
            .filter(|window| {
                window
                    .app_id
                    .as_deref()
                    .is_none_or(|id| id.trim().is_empty())
            })
            .count()
    }
    fn replace_windows(&mut self, windows: Vec<Window>) {
        self.windows = windows
            .into_iter()
            .map(|window| (window.id, window))
            .collect();
        if let Some(window) = self
            .windows
            .values()
            .find(|w| w.is_focused && self.allowed(w))
        {
            self.last_allowed = Some(window.id);
        }
    }
    pub fn refresh(&mut self, commands: &mut impl NiriCommands) -> Result<(), String> {
        self.replace_windows(commands.windows()?);
        Ok(())
    }
    fn apply(&mut self, event: Event) {
        match event {
            Event::WindowsChanged { windows } => self.replace_windows(windows),
            Event::WindowOpenedOrChanged { window } => {
                if window.is_focused && self.allowed(&window) {
                    self.last_allowed = Some(window.id);
                }
                self.windows.insert(window.id, window);
            }
            Event::WindowClosed { id } => {
                self.windows.remove(&id);
                self.attempted.remove(&id);
            }
            Event::WindowFocusChanged { id } => {
                if id
                    .and_then(|id| self.windows.get(&id))
                    .is_some_and(|w| self.allowed(w))
                {
                    self.last_allowed = id;
                }
            }
            Event::Other => {}
        }
    }
    pub fn enforce(&mut self, commands: &mut impl NiriCommands) -> Result<(), String> {
        let blocked = self
            .windows
            .values()
            .filter(|window| !self.allowed(window))
            .cloned()
            .collect::<Vec<_>>();
        let mut requested = false;
        for window in blocked {
            if !self.attempted.insert(window.id) {
                continue;
            }
            self.blocked_attempts = self.blocked_attempts.saturating_add(1);
            let result = commands.close(window.id);
            // A window may disappear between the event and the close request.
            self.refresh(commands)?;
            if result.is_err() && self.windows.contains_key(&window.id) {
                return Err("Niri could not request a blocked window close".into());
            }
            self.latest_notice = Some(format!(
                "{} is outside your session whitelist. A close was requested.",
                window.app_id.as_deref().unwrap_or("Application")
            ));
            requested = true;
        }
        if requested {
            if let Some(id) = self
                .last_allowed
                .filter(|id| self.windows.get(id).is_some_and(|w| self.allowed(w)))
            {
                // Focus restoration is explicitly best effort. Never target an arbitrary window.
                let _ = commands.focus(id);
            }
        }
        Ok(())
    }
    pub fn take_blocked_attempts(&mut self) -> u32 {
        std::mem::take(&mut self.blocked_attempts)
    }
}

struct EventStream {
    child: Option<ManagedChild>,
    receiver: Receiver<Result<Event, String>>,
    reader: Option<JoinHandle<()>>,
}
impl EventStream {
    fn start() -> Result<Self, String> {
        Self::from_command("niri", &["msg", "-j", "event-stream"])
    }
    fn from_command(program: &str, args: &[&str]) -> Result<Self, String> {
        let mut child = platform_command::spawn(program, args)?;
        let stdout = child
            .0
            .stdout
            .take()
            .ok_or("Niri event output unavailable")?;
        let (sender, receiver) = mpsc::sync_channel(256);
        let reader = std::thread::spawn(move || {
            let mut input = BufReader::new(stdout);
            loop {
                let mut line = Vec::new();
                let read = input
                    .by_ref()
                    .take((OUTPUT_LIMIT + 1) as u64)
                    .read_until(b'\n', &mut line);
                let result = match read {
                    Ok(0) | Err(_) => Err("Niri event stream disconnected".into()),
                    Ok(_) if line.len() > OUTPUT_LIMIT => {
                        Err("Niri event exceeded the size limit".into())
                    }
                    Ok(_) => parse_event(&line),
                };
                let failed = result.is_err();
                // A full queue is a health failure, never an unbounded allocation.
                if sender.try_send(result).is_err() || failed {
                    break;
                }
            }
        });
        Ok(Self {
            child: Some(child),
            receiver,
            reader: Some(reader),
        })
    }
}
impl Drop for EventStream {
    fn drop(&mut self) {
        drop(self.child.take());
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

#[derive(Default)]
pub struct NiriAdapter<C = NiriIpc> {
    pub policy: WindowPolicy,
    commands: C,
    stream: Option<EventStream>,
    healthy: bool,
    pub detail: String,
    pub last_checked: u64,
}
impl<C: NiriCommands> NiriAdapter<C> {
    pub fn with_commands(commands: C) -> Self {
        Self {
            policy: WindowPolicy::default(),
            commands,
            stream: None,
            healthy: false,
            detail: "Niri has not been checked".into(),
            last_checked: 0,
        }
    }
    pub fn refresh(&mut self) -> Result<(), String> {
        let result = self.policy.refresh(&mut self.commands);
        self.record(result)
    }
    fn record(&mut self, result: Result<(), String>) -> Result<(), String> {
        self.healthy = result.is_ok();
        self.last_checked = crate::domain::now_seconds();
        self.detail = match &result {
            Ok(()) if self.stream.is_some() => "Niri window monitoring active".into(),
            Ok(()) => "Niri window inventory available".into(),
            Err(error) => format!("Niri: {error}"),
        };
        result
    }
    pub fn poll(&mut self) -> Result<u32, String> {
        let result = self.poll_events();
        self.record(result)?;
        Ok(self.policy.take_blocked_attempts())
    }
    fn poll_events(&mut self) -> Result<(), String> {
        let stream = self
            .stream
            .as_ref()
            .ok_or("Niri event stream is not running")?;
        for _ in 0..256 {
            match stream.receiver.try_recv() {
                Ok(event) => {
                    self.policy.apply(event?);
                    self.policy.enforce(&mut self.commands)?;
                }
                Err(TryRecvError::Empty) => return Ok(()),
                Err(TryRecvError::Disconnected) => {
                    return Err("Niri event stream disconnected or overflowed".into())
                }
            }
        }
        Ok(())
    }
}
impl<C: NiriCommands> RestrictionStep for NiriAdapter<C> {
    fn component(&self) -> &'static str {
        "applications"
    }
    fn preflight(&mut self) -> Result<(), String> {
        self.refresh()?;
        if !self.policy.blocked_apps().is_empty() {
            return Err("preflight_blocked: close or whitelist the listed applications".into());
        }
        Ok(())
    }
    fn activate(&mut self) -> Result<(), String> {
        if self.stream.is_some() {
            return Ok(());
        }
        let result = (|| {
            let stream = EventStream::start()?;
            let deadline = Instant::now() + TIMEOUT;
            loop {
                let event = stream
                    .receiver
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                    .map_err(|_| "Niri initial window inventory timed out")??;
                let initial = matches!(event, Event::WindowsChanged { .. });
                self.policy.apply(event);
                if initial {
                    break;
                }
            }
            if !self.policy.blocked_apps().is_empty() {
                return Err(
                    "preflight_blocked: the window inventory changed during startup".into(),
                );
            }
            self.stream = Some(stream);
            Ok(())
        })();
        self.record(result)
    }
    fn deactivate(&mut self) -> Result<(), String> {
        self.stream = None;
        self.policy.attempted.clear();
        Ok(())
    }
    fn healthy(&self) -> bool {
        self.healthy
    }
}
impl<C: NiriCommands> ApplicationRestrictionAdapter for NiriAdapter<C> {}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct FakeNiri {
        windows: Vec<Window>,
        closes: Vec<u64>,
        focuses: Vec<u64>,
        refuse: bool,
        fail: bool,
    }
    impl NiriCommands for FakeNiri {
        fn windows(&mut self) -> Result<Vec<Window>, String> {
            Ok(self.windows.clone())
        }
        fn close(&mut self, id: u64) -> Result<(), String> {
            self.closes.push(id);
            if self.fail {
                return Err("failed".into());
            }
            if !self.refuse {
                self.windows.retain(|w| w.id != id);
            }
            Ok(())
        }
        fn focus(&mut self, id: u64) -> Result<(), String> {
            self.focuses.push(id);
            Ok(())
        }
    }
    fn window(id: u64, app_id: Option<&str>) -> Window {
        Window {
            id,
            app_id: app_id.map(str::to_owned),
            is_focused: id == 1,
        }
    }
    #[test]
    fn full_inventory_uses_exact_ids_and_allows_unknown_browser_and_system_windows() {
        let mut policy = WindowPolicy::default();
        policy.configure(vec!["Alacritty".into()]);
        policy.replace_windows(vec![
            window(1, Some("Alacritty")),
            window(2, Some("alacritty")),
            window(3, None),
            window(4, Some("")),
            window(5, Some("firefox")),
            window(6, Some("com.deepify.desktop")),
            window(7, Some("org.freedesktop.impl.portal.desktop.gtk")),
            window(8, Some("org.freedesktop.fake")),
        ]);
        assert_eq!(policy.blocked_apps(), ["alacritty", "org.freedesktop.fake"]);
        assert_eq!(policy.unidentified_count(), 2);
    }
    #[test]
    fn duplicate_title_changes_never_repeat_close_and_refused_close_stays_visible() {
        let mut policy = WindowPolicy::default();
        policy.configure(vec!["Alacritty".into()]);
        let mut ipc = FakeNiri {
            windows: vec![
                window(1, Some("Alacritty")),
                window(2, Some("chat")),
                window(3, Some("chat")),
            ],
            refuse: true,
            ..Default::default()
        };
        policy.refresh(&mut ipc).unwrap();
        policy.enforce(&mut ipc).unwrap();
        for _ in 0..10 {
            policy.apply(Event::WindowOpenedOrChanged {
                window: window(2, Some("chat")),
            });
            policy.enforce(&mut ipc).unwrap();
        }
        assert_eq!(ipc.closes, [2, 3]);
        assert_eq!(policy.take_blocked_attempts(), 2);
        assert_eq!(policy.take_blocked_attempts(), 0);
        assert_eq!(policy.blocked_apps(), ["chat"]);
        assert_eq!(ipc.focuses, [1]);
        policy.apply(Event::WindowClosed { id: 2 });
        policy.apply(Event::WindowOpenedOrChanged {
            window: window(2, Some("chat")),
        });
        policy.enforce(&mut ipc).unwrap();
        assert_eq!(ipc.closes, [2, 3, 2]);
    }
    #[test]
    fn closes_only_blocked_ids_and_reports_ipc_failure() {
        let mut policy = WindowPolicy::default();
        let mut ipc = FakeNiri {
            windows: vec![window(1, None), window(2, Some("chat"))],
            ..Default::default()
        };
        policy.refresh(&mut ipc).unwrap();
        policy.enforce(&mut ipc).unwrap();
        assert_eq!(ipc.closes, [2]);
        assert!(policy.blocked_apps().is_empty());
        ipc.windows.push(window(3, Some("game")));
        ipc.fail = true;
        policy.refresh(&mut ipc).unwrap();
        assert!(policy.enforce(&mut ipc).is_err());
    }
    #[test]
    fn ignores_unrelated_events_but_rejects_malformed_window_events() {
        assert!(matches!(
            parse_event(br#"{"WorkspaceActivated":{"id":1}}"#).unwrap(),
            Event::Other
        ));
        assert!(parse_event(br#"{"WindowOpenedOrChanged":{"window":{"id":1}}}"#).is_err());
    }

    #[test]
    fn process_stream_disconnect_and_invalid_events_invalidate_health() {
        for script in ["exit 0", "printf 'not-json\\n'"] {
            let mut adapter = NiriAdapter::with_commands(FakeNiri::default());
            adapter.stream = Some(EventStream::from_command("sh", &["-c", script]).unwrap());
            let started = Instant::now();
            while adapter.poll().is_ok() {
                assert!(started.elapsed() < TIMEOUT);
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            assert!(!adapter.healthy());
            adapter.deactivate().unwrap();
            assert!(adapter.stream.is_none());
        }
    }

    #[test]
    fn dropping_stream_stops_and_reaps_its_child() {
        let stream = EventStream::from_command("sh", &["-c", "exec sleep 30"]).unwrap();
        let pid = stream.child.as_ref().unwrap().0.id();
        drop(stream);
        // SAFETY: kill with signal 0 only queries whether this child PID exists.
        assert_eq!(unsafe { libc::kill(pid as i32, 0) }, -1);
    }
}
