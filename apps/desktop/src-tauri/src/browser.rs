//! User-only desktop broker for the paired Firefox/Zen extension.
//!
//! The browser protocol is intentionally URL-free. The extension keeps original
//! tab destinations in memory; this broker persists only a token hash and profile label.
use deepify_browser_protocol::{parse_frame, Envelope, MessageType, MAX_FRAME_BYTES, VERSION};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    env, fs,
    io::{self, Read, Write},
    os::{
        fd::AsRawFd,
        unix::{
            fs::{FileTypeExt, MetadataExt, PermissionsExt},
            net::{UnixListener, UnixStream},
        },
    },
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const SOCKET_DIRECTORY: &str = "deepify";
const SOCKET_NAME: &str = "browser-v1.sock";
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserPairing {
    pub token_hash: String,
    pub browser_kind: String,
    pub profile_label: String,
    pub paired_at: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserSnapshot {
    pub state: &'static str,
    pub browser_kind: Option<String>,
    pub profile_label: Option<String>,
    pub last_checked: Option<u64>,
    pub detail: &'static str,
}

#[derive(Clone, Debug)]
struct PendingProfile {
    token_hash: String,
    browser_kind: String,
    profile_label: String,
    hello_id: String,
}

struct ConnectedProfile {
    token_hash: String,
    writer: Arc<Mutex<UnixStream>>,
    last_checked: Instant,
    last_checked_epoch: u64,
    active_session: Option<String>,
}

#[derive(Default)]
struct BrokerState {
    paired: Option<BrowserPairing>,
    pending: Option<PendingProfile>,
    connected: Option<ConnectedProfile>,
    responses: HashMap<String, Envelope>,
    blocked_events: Vec<String>,
    seen_blocked_events: HashSet<String>,
    integration_errors: Vec<String>,
}

#[derive(Clone)]
pub struct BrowserBroker {
    state: Arc<Mutex<BrokerState>>,
    socket_path: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum BrowserError {
    #[error("browser integration is not paired")]
    Unpaired,
    #[error("browser integration is not connected")]
    Disconnected,
    #[error("browser integration timed out")]
    Timeout,
    #[error("browser integration rejected request")]
    Rejected,
    #[error("browser protocol error")]
    Protocol,
    #[error("browser socket error")]
    Socket,
}

fn epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn hash_token(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}
fn constant_time_hash_match(left: &str, right: &str) -> bool {
    // SHA-256 hex strings have a fixed public length. Do not short-circuit on a mismatch.
    let mut difference = left.len() ^ right.len();
    let width = left.len().max(right.len());
    for index in 0..width {
        difference |= usize::from(
            *left.as_bytes().get(index).unwrap_or(&0) ^ *right.as_bytes().get(index).unwrap_or(&0),
        );
    }
    difference == 0
}

fn socket_path() -> io::Result<PathBuf> {
    let runtime = env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "XDG_RUNTIME_DIR is required"))?;
    let uid = unsafe { libc::geteuid() };
    let metadata = fs::metadata(&runtime)?;
    if !metadata.is_dir() || metadata.uid() != uid || metadata.permissions().mode() & 0o077 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "insecure runtime directory",
        ));
    }
    let directory = runtime.join(SOCKET_DIRECTORY);
    fs::create_dir_all(&directory)?;
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
    Ok(directory.join(SOCKET_NAME))
}

fn prepare_listener(path: &PathBuf) -> io::Result<UnixListener> {
    if path.exists() {
        let metadata = fs::symlink_metadata(path)?;
        if metadata.uid() != unsafe { libc::geteuid() } || !metadata.file_type().is_socket() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "unsafe browser socket",
            ));
        }
        if UnixStream::connect(path).is_ok() {
            return Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                "browser socket already has a listener",
            ));
        }
        fs::remove_file(path)?;
    }
    let listener = UnixListener::bind(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(listener)
}

fn peer_is_current_uid(stream: &UnixStream) -> bool {
    #[cfg(target_os = "linux")]
    {
        let mut credential: libc::ucred = unsafe { std::mem::zeroed() };
        let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        let result = unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                &mut credential as *mut _ as *mut libc::c_void,
                &mut length,
            )
        };
        result == 0 && credential.uid == unsafe { libc::geteuid() }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = stream;
        false
    }
}

fn read_frame(reader: &mut impl Read) -> io::Result<Option<Vec<u8>>> {
    let mut length = [0; 4];
    let mut first = [0; 1];
    if reader.read(&mut first)? == 0 {
        return Ok(None);
    }
    length[0] = first[0];
    reader.read_exact(&mut length[1..])?;
    let size = u32::from_le_bytes(length) as usize;
    if size > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "oversized frame",
        ));
    }
    let mut body = vec![0; size];
    reader.read_exact(&mut body)?;
    Ok(Some(body))
}
fn write_envelope(writer: &mut impl Write, message: &Envelope) -> io::Result<()> {
    let body = serde_json::to_vec(message)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "message serialization"))?;
    if body.len() > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "oversized frame",
        ));
    }
    writer.write_all(&(body.len() as u32).to_le_bytes())?;
    writer.write_all(&body)?;
    writer.flush()
}
fn envelope(
    message_type: MessageType,
    payload: BTreeMap<String, Value>,
    request_id: Option<String>,
) -> Envelope {
    Envelope {
        version: VERSION,
        message_type,
        message_id: uuid::Uuid::new_v4().to_string(),
        request_id,
        payload,
    }
}
fn text(message: &Envelope, key: &str) -> Option<String> {
    message
        .payload
        .get(key)?
        .as_str()
        .filter(|value| !value.is_empty() && value.len() <= 128)
        .map(str::to_owned)
}

fn healthy_state(message: &Envelope) -> bool {
    matches!(
        text(message, "health").as_deref(),
        Some("healthy_idle") | Some("active")
    )
}

impl BrowserBroker {
    pub fn start(pairing: Option<BrowserPairing>) -> Result<Self, BrowserError> {
        let socket_path = socket_path().map_err(|_| BrowserError::Socket)?;
        let listener = prepare_listener(&socket_path).map_err(|_| BrowserError::Socket)?;
        let broker = Self {
            state: Arc::new(Mutex::new(BrokerState {
                paired: pairing,
                ..Default::default()
            })),
            socket_path,
        };
        let receiver = broker.clone();
        thread::Builder::new()
            .name("deepify-browser-broker".into())
            .spawn(move || {
                for incoming in listener.incoming() {
                    let Ok(stream) = incoming else {
                        break;
                    };
                    if !peer_is_current_uid(&stream) {
                        continue;
                    }
                    let broker = receiver.clone();
                    let _ = thread::Builder::new()
                        .name("deepify-browser-peer".into())
                        .spawn(move || broker.serve(stream));
                }
            })
            .map_err(|_| BrowserError::Socket)?;
        Ok(broker)
    }

    fn serve(&self, mut reader: UnixStream) {
        let Ok(writer_stream) = reader.try_clone() else {
            return;
        };
        let writer = Arc::new(Mutex::new(writer_stream));
        let mut connected_hash = None;
        while let Ok(Some(bytes)) = read_frame(&mut reader) {
            let Ok(message) = parse_frame(&bytes) else {
                break;
            };
            if connected_hash.is_none() {
                if message.message_type != MessageType::Hello {
                    break;
                }
                let Some(token) = text(&message, "token") else {
                    break;
                };
                if text(&message, "extension_id").as_deref() != Some("focus@deepify.local") {
                    break;
                }
                let Some(browser_kind) = text(&message, "browser_kind")
                    .filter(|value| value == "firefox" || value == "zen")
                else {
                    break;
                };
                let Some(profile_label) = text(&message, "profile_label") else {
                    break;
                };
                let token_hash = hash_token(&token);
                let acceptance = self.hello(
                    token_hash.clone(),
                    browser_kind,
                    profile_label,
                    message.message_id.clone(),
                    writer.clone(),
                );
                let response = envelope(
                    MessageType::PairResult,
                    BTreeMap::from([
                        ("accepted".to_owned(), Value::Bool(acceptance)),
                        (
                            "error_code".to_owned(),
                            Value::String(
                                if acceptance { "internal" } else { "unpaired" }.to_owned(),
                            ),
                        ),
                    ]),
                    Some(message.message_id),
                );
                let _ = write_envelope(&mut *writer.lock().unwrap(), &response);
                connected_hash = Some(token_hash);
                continue;
            }
            self.handle(message, connected_hash.as_deref().unwrap());
        }
        if let Some(token_hash) = connected_hash {
            if let Ok(mut state) = self.state.lock() {
                if state
                    .connected
                    .as_ref()
                    .is_some_and(|connection| connection.token_hash == token_hash)
                {
                    state.connected = None;
                }
            }
        }
    }

    fn hello(
        &self,
        token_hash: String,
        browser_kind: String,
        profile_label: String,
        hello_id: String,
        writer: Arc<Mutex<UnixStream>>,
    ) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        let accepted = if let Some(pairing) = state.paired.as_ref() {
            if !constant_time_hash_match(&pairing.token_hash, &token_hash) {
                return false;
            }
            true
        } else {
            if let Some(pending) = state.pending.as_ref() {
                if !constant_time_hash_match(&pending.token_hash, &token_hash) {
                    return false;
                }
            } else {
                state.pending = Some(PendingProfile {
                    token_hash: token_hash.clone(),
                    browser_kind,
                    profile_label,
                    hello_id,
                });
            }
            false
        };
        state.connected = Some(ConnectedProfile {
            token_hash,
            writer,
            last_checked: Instant::now(),
            last_checked_epoch: epoch_seconds(),
            active_session: None,
        });
        accepted
    }

    fn handle(&self, message: Envelope, token_hash: &str) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        if state
            .connected
            .as_ref()
            .is_none_or(|connection| !constant_time_hash_match(&connection.token_hash, token_hash))
        {
            return;
        }
        if let Some(connection) = state.connected.as_mut() {
            connection.last_checked = Instant::now();
            connection.last_checked_epoch = epoch_seconds();
        }
        match message.message_type {
            MessageType::BlockedAttempt => {
                if let Some(session_id) = text(&message, "session_id") {
                    if state.seen_blocked_events.insert(message.message_id) {
                        state.blocked_events.push(session_id);
                    }
                    if state.seen_blocked_events.len() > 4096 {
                        state.seen_blocked_events.clear();
                    }
                }
            }
            MessageType::IntegrationError | MessageType::RestoreError => state
                .integration_errors
                .push("extension reported an integration error".into()),
            _ if message.request_id.is_some() => {
                state
                    .responses
                    .insert(message.request_id.clone().unwrap(), message);
            }
            _ => {}
        }
    }

    pub fn accept_pending(&self) -> Result<BrowserPairing, BrowserError> {
        let mut state = self.state.lock().map_err(|_| BrowserError::Socket)?;
        let pending = state.pending.take().ok_or(BrowserError::Unpaired)?;
        let pairing = BrowserPairing {
            token_hash: pending.token_hash,
            browser_kind: pending.browser_kind,
            profile_label: pending.profile_label,
            paired_at: epoch_seconds(),
        };
        state.paired = Some(pairing.clone());
        if let Some(connection) = state.connected.as_ref() {
            let response = envelope(
                MessageType::PairResult,
                BTreeMap::from([("accepted".to_owned(), Value::Bool(true))]),
                Some(pending.hello_id),
            );
            let _ = write_envelope(
                &mut *connection.writer.lock().map_err(|_| BrowserError::Socket)?,
                &response,
            );
        }
        Ok(pairing)
    }
    pub fn forget_pairing(&self) -> Result<(), BrowserError> {
        let mut state = self.state.lock().map_err(|_| BrowserError::Socket)?;
        if state
            .connected
            .as_ref()
            .is_some_and(|connection| connection.active_session.is_some())
        {
            return Err(BrowserError::Rejected);
        }
        state.paired = None;
        state.pending = None;
        Ok(())
    }
    pub fn snapshot(&self) -> BrowserSnapshot {
        let Ok(state) = self.state.lock() else {
            return BrowserSnapshot {
                state: "unhealthy",
                browser_kind: None,
                profile_label: None,
                last_checked: None,
                detail: "Browser broker unavailable",
            };
        };
        if let Some(pending) = state.pending.as_ref() {
            return BrowserSnapshot {
                state: "pending_pair",
                browser_kind: Some(pending.browser_kind.clone()),
                profile_label: Some(pending.profile_label.clone()),
                last_checked: None,
                detail: "Pairing required",
            };
        }
        if let Some(connection) = state.connected.as_ref() {
            let paired = state.paired.as_ref();
            return BrowserSnapshot {
                state: if connection.active_session.is_some() {
                    "active"
                } else {
                    "healthy_idle"
                },
                browser_kind: paired.map(|item| item.browser_kind.clone()),
                profile_label: paired.map(|item| item.profile_label.clone()),
                last_checked: Some(connection.last_checked_epoch),
                detail: "Paired profile connected",
            };
        }
        if let Some(pairing) = state.paired.as_ref() {
            return BrowserSnapshot {
                state: "unhealthy",
                browser_kind: Some(pairing.browser_kind.clone()),
                profile_label: Some(pairing.profile_label.clone()),
                last_checked: None,
                detail: "Paired profile disconnected",
            };
        }
        BrowserSnapshot {
            state: "unpaired",
            browser_kind: None,
            profile_label: None,
            last_checked: None,
            detail: "Install and connect the Deepify extension",
        }
    }
    fn request(
        &self,
        kind: MessageType,
        payload: BTreeMap<String, Value>,
    ) -> Result<Envelope, BrowserError> {
        let message = envelope(kind, payload, None);
        let request_id = message.message_id.clone();
        let writer = {
            let state = self.state.lock().map_err(|_| BrowserError::Socket)?;
            let pairing = state.paired.as_ref().ok_or(BrowserError::Unpaired)?;
            let connection = state.connected.as_ref().ok_or(BrowserError::Disconnected)?;
            if !constant_time_hash_match(&pairing.token_hash, &connection.token_hash) {
                return Err(BrowserError::Unpaired);
            }
            connection.writer.clone()
        };
        write_envelope(
            &mut *writer.lock().map_err(|_| BrowserError::Socket)?,
            &message,
        )
        .map_err(|_| BrowserError::Socket)?;
        let deadline = Instant::now() + RESPONSE_TIMEOUT;
        while Instant::now() < deadline {
            if let Ok(mut state) = self.state.lock() {
                if let Some(response) = state.responses.remove(&request_id) {
                    return Ok(response);
                }
            }
            thread::sleep(Duration::from_millis(10));
        }
        Err(BrowserError::Timeout)
    }
    pub fn check_health(&self) -> Result<(), BrowserError> {
        let response = self.request(MessageType::Status, BTreeMap::new())?;
        if response.message_type != MessageType::State || !healthy_state(&response) {
            return Err(BrowserError::Rejected);
        }
        Ok(())
    }
    pub fn heartbeat(&self) -> Result<(), BrowserError> {
        let response = self.request(MessageType::Heartbeat, BTreeMap::new())?;
        if response.message_type == MessageType::HeartbeatAck {
            Ok(())
        } else {
            Err(BrowserError::Rejected)
        }
    }
    pub fn start_session(
        &self,
        session_id: &str,
        rules: Vec<(String, String)>,
        timer_state: &str,
        remaining_seconds: u64,
    ) -> Result<(), BrowserError> {
        let rules = rules
            .into_iter()
            .map(|(host, path)| json!({"host":host,"path":path}))
            .collect();
        let response = self.request(
            MessageType::StartSession,
            BTreeMap::from([
                ("session_id".into(), Value::String(session_id.into())),
                ("rules".into(), Value::Array(rules)),
                ("timer_state".into(), Value::String(timer_state.into())),
                ("remaining_seconds".into(), Value::from(remaining_seconds)),
            ]),
        )?;
        if response.message_type != MessageType::StartResult
            || response.payload.get("accepted") != Some(&Value::Bool(true))
            || text(&response, "session_id").as_deref() != Some(session_id)
        {
            return Err(BrowserError::Rejected);
        }
        if let Ok(mut state) = self.state.lock() {
            if let Some(connection) = state.connected.as_mut() {
                connection.active_session = Some(session_id.into());
            }
        }
        Ok(())
    }
    pub fn stop_session(&self, session_id: &str) -> Result<(), BrowserError> {
        let response = self.request(
            MessageType::StopSession,
            BTreeMap::from([("session_id".into(), Value::String(session_id.into()))]),
        )?;
        if response.message_type != MessageType::StopResult
            || response.payload.get("accepted") != Some(&Value::Bool(true))
            || text(&response, "session_id").as_deref() != Some(session_id)
        {
            return Err(BrowserError::Rejected);
        }
        if let Ok(mut state) = self.state.lock() {
            if let Some(connection) = state.connected.as_mut() {
                connection.active_session = None;
            }
        }
        Ok(())
    }
    /// Resolve a persisted desktop-restart checkpoint using a fresh extension
    /// state response. An inactive extension proves cleanup; an active one must
    /// restore the exact abandoned session before another start is permitted.
    pub fn recover_cleanup(&self, session_id: &str) -> Result<(), BrowserError> {
        let state = self.request(MessageType::Status, BTreeMap::new())?;
        if state.message_type != MessageType::State {
            return Err(BrowserError::Rejected);
        }
        match text(&state, "health").as_deref() {
            Some("healthy_idle") => Ok(()),
            Some("active") if text(&state, "session_id").as_deref() == Some(session_id) => {
                self.stop_session(session_id)
            }
            _ => Err(BrowserError::Rejected),
        }
    }
    pub fn sync_timer_state(
        &self,
        session_id: &str,
        timer_state: &str,
        remaining_seconds: u64,
    ) -> Result<(), BrowserError> {
        let writer = self
            .state
            .lock()
            .map_err(|_| BrowserError::Socket)?
            .connected
            .as_ref()
            .ok_or(BrowserError::Disconnected)?
            .writer
            .clone();
        let message = envelope(
            MessageType::State,
            BTreeMap::from([
                ("session_id".into(), Value::String(session_id.into())),
                ("health".into(), Value::String("active".into())),
                ("timer_state".into(), Value::String(timer_state.into())),
                ("remaining_seconds".into(), Value::from(remaining_seconds)),
            ]),
            None,
        );
        let mut stream = writer.lock().map_err(|_| BrowserError::Socket)?;
        write_envelope(&mut *stream, &message).map_err(|_| BrowserError::Socket)
    }
    pub fn take_blocked_events(&self) -> Vec<String> {
        self.state
            .lock()
            .map(|mut state| std::mem::take(&mut state.blocked_events))
            .unwrap_or_default()
    }
    pub fn take_integration_errors(&self) -> Vec<String> {
        self.state
            .lock()
            .map(|mut state| std::mem::take(&mut state.integration_errors))
            .unwrap_or_default()
    }
    pub fn socket_path(&self) -> &PathBuf {
        &self.socket_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn test_broker(pairing: Option<BrowserPairing>) -> BrowserBroker {
        BrowserBroker {
            state: Arc::new(Mutex::new(BrokerState {
                paired: pairing,
                ..Default::default()
            })),
            socket_path: PathBuf::from("/unused/deepify-browser-v1.sock"),
        }
    }

    fn test_writer() -> Arc<Mutex<UnixStream>> {
        let (writer, _peer) = UnixStream::pair().unwrap();
        Arc::new(Mutex::new(writer))
    }

    fn paired_broker_with_peer() -> (BrowserBroker, String, UnixStream) {
        let token_hash = hash_token("paired profile token");
        let (writer, peer) = UnixStream::pair().unwrap();
        let broker = test_broker(Some(BrowserPairing {
            token_hash: token_hash.clone(),
            browser_kind: "firefox".into(),
            profile_label: "Primary profile".into(),
            paired_at: 1,
        }));
        broker.state.lock().unwrap().connected = Some(ConnectedProfile {
            token_hash: token_hash.clone(),
            writer: Arc::new(Mutex::new(writer)),
            last_checked: Instant::now(),
            last_checked_epoch: 1,
            active_session: None,
        });
        (broker, token_hash, peer)
    }

    fn blocked_event(message_id: &str, session_id: &str) -> Envelope {
        Envelope {
            version: VERSION,
            message_type: MessageType::BlockedAttempt,
            message_id: message_id.into(),
            request_id: None,
            payload: BTreeMap::from([("session_id".into(), Value::String(session_id.into()))]),
        }
    }

    #[test]
    fn token_hashes_are_not_reversible_and_compare_in_constant_time() {
        let hash = hash_token("not a raw pairing token");
        assert_ne!(hash, "not a raw pairing token");
        assert!(constant_time_hash_match(&hash, &hash));
        assert!(!constant_time_hash_match(&hash, &hash_token("different")));
    }
    #[test]
    fn broker_snapshot_does_not_expose_a_token() {
        let snapshot = BrowserSnapshot {
            state: "unpaired",
            browser_kind: None,
            profile_label: None,
            last_checked: None,
            detail: "Install and connect the Deepify extension",
        };
        assert!(!format!("{snapshot:?}").contains("token"));
    }
    #[test]
    fn only_active_or_idle_extension_states_are_healthy() {
        let state = |health: &str| Envelope {
            version: VERSION,
            message_type: MessageType::State,
            message_id: "message".into(),
            request_id: Some("request".into()),
            payload: BTreeMap::from([
                ("health".into(), Value::String(health.into())),
                ("timer_state".into(), Value::String("inactive".into())),
                ("remaining_seconds".into(), Value::from(0)),
            ]),
        };
        assert!(healthy_state(&state("healthy_idle")));
        assert!(healthy_state(&state("active")));
        assert!(!healthy_state(&state("pending_pair")));
        assert!(!healthy_state(&state("cleanup_required")));
        assert!(!healthy_state(&state("unhealthy")));
    }
    #[test]
    fn pairing_rejects_a_second_profile_and_cannot_be_forgotten_while_active() {
        let broker = test_broker(None);
        let first = hash_token("first profile token");
        assert!(!broker.hello(
            first.clone(),
            "firefox".into(),
            "Primary profile".into(),
            "hello-first".into(),
            test_writer(),
        ));
        assert_eq!(broker.snapshot().state, "pending_pair");
        let pairing = broker.accept_pending().unwrap();
        assert_eq!(pairing.token_hash, first);
        assert_eq!(broker.snapshot().state, "healthy_idle");
        assert!(!broker.hello(
            hash_token("second profile token"),
            "firefox".into(),
            "Second profile".into(),
            "hello-second".into(),
            test_writer(),
        ));
        assert_eq!(
            broker.snapshot().profile_label.as_deref(),
            Some("Primary profile")
        );
        broker
            .state
            .lock()
            .unwrap()
            .connected
            .as_mut()
            .unwrap()
            .active_session = Some("session-1".into());
        assert!(matches!(
            broker.forget_pairing(),
            Err(BrowserError::Rejected)
        ));
    }
    #[test]
    fn blocked_attempts_are_deduplicated_and_ignore_an_unknown_connection() {
        let token_hash = hash_token("paired profile token");
        let broker = test_broker(Some(BrowserPairing {
            token_hash: token_hash.clone(),
            browser_kind: "firefox".into(),
            profile_label: "Primary profile".into(),
            paired_at: 1,
        }));
        broker.state.lock().unwrap().connected = Some(ConnectedProfile {
            token_hash: token_hash.clone(),
            writer: test_writer(),
            last_checked: Instant::now(),
            last_checked_epoch: 1,
            active_session: Some("session-1".into()),
        });
        broker.handle(blocked_event("event-1", "session-1"), &token_hash);
        broker.handle(blocked_event("event-1", "session-1"), &token_hash);
        broker.handle(blocked_event("event-2", "session-2"), "unknown-token");
        assert_eq!(broker.take_blocked_events(), vec!["session-1"]);
    }
    #[test]
    fn recovery_accepts_idle_or_stops_the_exact_abandoned_session() {
        let (broker, token_hash, mut peer) = paired_broker_with_peer();
        let responder = {
            let broker = broker.clone();
            thread::spawn(move || {
                let request = parse_frame(&read_frame(&mut peer).unwrap().unwrap()).unwrap();
                assert_eq!(request.message_type, MessageType::Status);
                broker.handle(
                    Envelope {
                        version: VERSION,
                        message_type: MessageType::State,
                        message_id: "state".into(),
                        request_id: Some(request.message_id),
                        payload: BTreeMap::from([
                            ("health".into(), Value::String("active".into())),
                            ("session_id".into(), Value::String("abandoned".into())),
                            ("timer_state".into(), Value::String("paused".into())),
                            ("remaining_seconds".into(), Value::from(30)),
                        ]),
                    },
                    &token_hash,
                );
                let stop = parse_frame(&read_frame(&mut peer).unwrap().unwrap()).unwrap();
                assert_eq!(stop.message_type, MessageType::StopSession);
                assert_eq!(text(&stop, "session_id").as_deref(), Some("abandoned"));
                broker.handle(
                    Envelope {
                        version: VERSION,
                        message_type: MessageType::StopResult,
                        message_id: "stopped".into(),
                        request_id: Some(stop.message_id),
                        payload: BTreeMap::from([
                            ("session_id".into(), Value::String("abandoned".into())),
                            ("accepted".into(), Value::Bool(true)),
                            ("restored_count".into(), Value::from(1)),
                        ]),
                    },
                    &token_hash,
                );
            })
        };
        assert!(broker.recover_cleanup("abandoned").is_ok());
        responder.join().unwrap();

        let (broker, token_hash, mut peer) = paired_broker_with_peer();
        let responder = {
            let broker = broker.clone();
            thread::spawn(move || {
                let request = parse_frame(&read_frame(&mut peer).unwrap().unwrap()).unwrap();
                broker.handle(
                    Envelope {
                        version: VERSION,
                        message_type: MessageType::State,
                        message_id: "idle".into(),
                        request_id: Some(request.message_id),
                        payload: BTreeMap::from([
                            ("health".into(), Value::String("healthy_idle".into())),
                            ("timer_state".into(), Value::String("inactive".into())),
                            ("remaining_seconds".into(), Value::from(0)),
                        ]),
                    },
                    &token_hash,
                );
            })
        };
        assert!(broker.recover_cleanup("abandoned").is_ok());
        responder.join().unwrap();
    }
    #[test]
    fn integration_errors_are_url_free_and_available_to_the_session_loop() {
        let (broker, token_hash, _peer) = paired_broker_with_peer();
        broker.handle(
            Envelope {
                version: VERSION,
                message_type: MessageType::IntegrationError,
                message_id: "error".into(),
                request_id: None,
                payload: BTreeMap::from([(
                    "error_code".into(),
                    Value::String("permission_lost".into()),
                )]),
            },
            &token_hash,
        );
        assert_eq!(
            broker.take_integration_errors(),
            vec!["extension reported an integration error"]
        );
        assert!(broker.take_integration_errors().is_empty());
    }
}
