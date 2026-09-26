//! Strict, URL-free version-one messages shared by the desktop and native host.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

pub const VERSION: u8 = 1;
pub const MAX_FRAME_BYTES: usize = 256 * 1024;

#[derive(Debug, thiserror::Error, Eq, PartialEq)]
pub enum ProtocolError {
    #[error("invalid browser message")]
    InvalidMessage,
    #[error("unsupported browser protocol version")]
    UnsupportedVersion,
    #[error("browser message is too large")]
    Oversized,
}

/// A validated envelope. `parse_frame` limits payload keys by message type.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Envelope {
    pub version: u8,
    #[serde(rename = "type")]
    pub message_type: MessageType,
    pub message_id: String,
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(flatten)]
    pub payload: std::collections::BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    Hello,
    Pair,
    PairResult,
    Heartbeat,
    HeartbeatAck,
    StartSession,
    StartResult,
    StopSession,
    StopResult,
    Status,
    State,
    BlockedAttempt,
    IntegrationError,
    RestoreComplete,
    RestoreError,
}

impl MessageType {
    pub fn is_response(&self) -> bool {
        matches!(
            self,
            Self::PairResult | Self::HeartbeatAck | Self::StartResult | Self::StopResult
        )
    }
}

pub fn parse_frame(bytes: &[u8]) -> Result<Envelope, ProtocolError> {
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(ProtocolError::Oversized);
    }
    let envelope: Envelope =
        serde_json::from_slice(bytes).map_err(|_| ProtocolError::InvalidMessage)?;
    if envelope.version != VERSION {
        return Err(ProtocolError::UnsupportedVersion);
    }
    if envelope.message_id.is_empty()
        || envelope.message_id.len() > 128
        || envelope.message_id.chars().any(char::is_control)
    {
        return Err(ProtocolError::InvalidMessage);
    }
    if envelope.message_type.is_response()
        && envelope.request_id.as_deref().is_none_or(str::is_empty)
    {
        return Err(ProtocolError::InvalidMessage);
    }
    if envelope
        .request_id
        .as_ref()
        .is_some_and(|id| id.is_empty() || id.len() > 128 || id.chars().any(char::is_control))
    {
        return Err(ProtocolError::InvalidMessage);
    }
    if envelope
        .payload
        .keys()
        .any(|key| key == "url" || key == "destination" || key == "title" || key == "query")
    {
        return Err(ProtocolError::InvalidMessage);
    }
    let (allowed, required): (&[&str], &[&str]) = match envelope.message_type {
        MessageType::Hello => (
            [
                "token",
                "extension_id",
                "browser_kind",
                "profile_label",
                "capabilities",
            ]
            .as_slice(),
            [
                "token",
                "extension_id",
                "browser_kind",
                "profile_label",
                "capabilities",
            ]
            .as_slice(),
        ),
        MessageType::Pair => (["token"].as_slice(), ["token"].as_slice()),
        MessageType::PairResult => (
            ["accepted", "error_code"].as_slice(),
            ["accepted"].as_slice(),
        ),
        MessageType::Heartbeat | MessageType::HeartbeatAck | MessageType::Status => (&[], &[]),
        MessageType::StartSession => (
            ["session_id", "timer_state", "remaining_seconds", "rules"].as_slice(),
            ["session_id", "timer_state", "remaining_seconds", "rules"].as_slice(),
        ),
        MessageType::StartResult => (
            ["session_id", "accepted", "blocked_count", "error_code"].as_slice(),
            ["session_id", "accepted"].as_slice(),
        ),
        MessageType::StopSession => (["session_id"].as_slice(), ["session_id"].as_slice()),
        MessageType::StopResult => (
            ["session_id", "accepted", "restored_count", "error_code"].as_slice(),
            ["session_id", "accepted"].as_slice(),
        ),
        MessageType::State => (
            ["health", "session_id", "timer_state", "remaining_seconds"].as_slice(),
            ["health", "timer_state", "remaining_seconds"].as_slice(),
        ),
        MessageType::BlockedAttempt => (["session_id"].as_slice(), ["session_id"].as_slice()),
        MessageType::IntegrationError | MessageType::RestoreError => (
            ["session_id", "error_code"].as_slice(),
            ["error_code"].as_slice(),
        ),
        MessageType::RestoreComplete => (
            ["session_id", "restored_count"].as_slice(),
            ["session_id", "restored_count"].as_slice(),
        ),
    };
    if envelope
        .payload
        .keys()
        .any(|key| !allowed.contains(&key.as_str()))
        || required
            .iter()
            .any(|key| !envelope.payload.contains_key(*key))
    {
        return Err(ProtocolError::InvalidMessage);
    }
    if !payload_is_valid(&envelope) {
        return Err(ProtocolError::InvalidMessage);
    }
    Ok(envelope)
}

fn safe_text(value: Option<&Value>, minimum: usize, maximum: usize) -> bool {
    value.and_then(Value::as_str).is_some_and(|text| {
        text.len() >= minimum && text.len() <= maximum && !text.chars().any(char::is_control)
    })
}

fn optional_safe_text(value: Option<&Value>, minimum: usize, maximum: usize) -> bool {
    value.is_none_or(|value| safe_text(Some(value), minimum, maximum))
}

fn one_of(value: Option<&Value>, allowed: &[&str]) -> bool {
    value
        .and_then(Value::as_str)
        .is_some_and(|text| allowed.contains(&text))
}

fn unsigned(value: Option<&Value>, maximum: u64) -> bool {
    value
        .and_then(Value::as_u64)
        .is_some_and(|number| number <= maximum)
}

fn valid_rules(value: Option<&Value>) -> bool {
    let Some(rules) = value.and_then(Value::as_array) else {
        return false;
    };
    rules.len() <= 1024
        && rules.iter().all(|rule| {
            let Some(rule) = rule.as_object() else {
                return false;
            };
            rule.len() == 2
                && rule.contains_key("host")
                && rule.contains_key("path")
                && safe_text(rule.get("host"), 1, 253)
                && safe_text(rule.get("path"), 1, 2048)
        })
}

fn valid_capabilities(value: Option<&Value>) -> bool {
    let Some(capabilities) = value.and_then(Value::as_array) else {
        return false;
    };
    let allowed = ["top_level_web_request", "tab_restore", "container_tabs"];
    let mut seen = HashSet::new();
    capabilities.len() <= 16
        && capabilities.iter().all(|item| {
            item.as_str()
                .filter(|capability| allowed.contains(capability) && seen.insert(*capability))
                .is_some()
        })
}

fn payload_is_valid(envelope: &Envelope) -> bool {
    let payload = &envelope.payload;
    let text = |key, minimum, maximum| safe_text(payload.get(key), minimum, maximum);
    let optional_text =
        |key, minimum, maximum| optional_safe_text(payload.get(key), minimum, maximum);
    let error_codes = [
        "unpaired",
        "rejected",
        "invalid_request",
        "permission_lost",
        "session_mismatch",
        "restore_failed",
        "internal",
    ];
    match envelope.message_type {
        MessageType::Hello => {
            text("token", 43, 128)
                && payload.get("extension_id") == Some(&Value::String("focus@deepify.local".into()))
                && one_of(payload.get("browser_kind"), &["firefox", "zen"])
                && text("profile_label", 1, 128)
                && valid_capabilities(payload.get("capabilities"))
        }
        MessageType::Pair => text("token", 43, 128),
        MessageType::PairResult => {
            payload.get("accepted").is_some_and(Value::is_boolean)
                && payload
                    .get("error_code")
                    .is_none_or(|value| one_of(Some(value), &error_codes))
        }
        MessageType::Heartbeat | MessageType::HeartbeatAck | MessageType::Status => true,
        MessageType::StartSession => {
            text("session_id", 1, 128)
                && one_of(
                    payload.get("timer_state"),
                    &["working", "paused", "inactive"],
                )
                && unsigned(payload.get("remaining_seconds"), 86_400)
                && valid_rules(payload.get("rules"))
        }
        MessageType::StartResult => {
            text("session_id", 1, 128)
                && payload.get("accepted").is_some_and(Value::is_boolean)
                && payload
                    .get("blocked_count")
                    .is_none_or(|value| unsigned(Some(value), u32::MAX as u64))
                && payload
                    .get("error_code")
                    .is_none_or(|value| one_of(Some(value), &error_codes))
        }
        MessageType::StopSession => text("session_id", 1, 128),
        MessageType::StopResult => {
            text("session_id", 1, 128)
                && payload.get("accepted").is_some_and(Value::is_boolean)
                && payload
                    .get("restored_count")
                    .is_none_or(|value| unsigned(Some(value), u32::MAX as u64))
                && payload
                    .get("error_code")
                    .is_none_or(|value| one_of(Some(value), &error_codes))
        }
        MessageType::State => {
            one_of(
                payload.get("health"),
                &[
                    "pending_pair",
                    "healthy_idle",
                    "active",
                    "unhealthy",
                    "cleanup_required",
                ],
            ) && optional_text("session_id", 1, 128)
                && one_of(
                    payload.get("timer_state"),
                    &["working", "paused", "inactive"],
                )
                && unsigned(payload.get("remaining_seconds"), 86_400)
        }
        MessageType::BlockedAttempt => text("session_id", 1, 128),
        MessageType::IntegrationError | MessageType::RestoreError => {
            optional_text("session_id", 1, 128) && one_of(payload.get("error_code"), &error_codes)
        }
        MessageType::RestoreComplete => {
            text("session_id", 1, 128) && unsigned(payload.get("restored_count"), u32::MAX as u64)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unknown_fields_and_response_without_correlation() {
        assert_eq!(
            parse_frame(br#"{"version":1,"type":"heartbeat","message_id":"a","extra":true}"#),
            Err(ProtocolError::InvalidMessage)
        );
        assert_eq!(
            parse_frame(br#"{"version":1,"type":"heartbeat_ack","message_id":"a"}"#),
            Err(ProtocolError::InvalidMessage)
        );
    }
    #[test]
    fn correlation_ids_are_bounded_and_control_free() {
        for id in ["x".repeat(129), "line\nbreak".into(), String::new()] {
            let message = serde_json::json!({"version":1,"type":"heartbeat_ack","message_id":"reply","request_id":id});
            assert_eq!(
                parse_frame(&serde_json::to_vec(&message).unwrap()),
                Err(ProtocolError::InvalidMessage)
            );
        }
    }
    #[test]
    fn rejects_url_bearing_payloads_and_versions() {
        assert_eq!(
            parse_frame(br#"{"version":2,"type":"heartbeat","message_id":"a"}"#),
            Err(ProtocolError::UnsupportedVersion)
        );
        assert_eq!(parse_frame(br#"{"version":1,"type":"blocked_attempt","message_id":"a","url":"https://private.example"}"#), Err(ProtocolError::InvalidMessage));
    }
    #[test]
    fn accepts_a_valid_hello_payload() {
        assert!(parse_frame(br#"{"version":1,"type":"hello","message_id":"a","token":"MDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWY=","extension_id":"focus@deepify.local","browser_kind":"firefox","profile_label":"profile","capabilities":[]}"#).is_ok());
    }
    #[test]
    fn rejects_invalid_payload_types_enums_and_rule_shapes() {
        for message in [
            br#"{"version":1,"type":"hello","message_id":"a","token":7,"extension_id":"focus@deepify.local","browser_kind":"firefox","profile_label":"profile","capabilities":[]}"#.as_slice(),
            br#"{"version":1,"type":"state","message_id":"a","request_id":"b","health":"ready","timer_state":"inactive","remaining_seconds":0}"#.as_slice(),
            br#"{"version":1,"type":"start_session","message_id":"a","session_id":"s","timer_state":"working","remaining_seconds":1,"rules":[{"host":"example.com","path":"/","extra":true}]}"#.as_slice(),
            br#"{"version":1,"type":"start_result","message_id":"a","request_id":"b","session_id":"s","accepted":"yes"}"#.as_slice(),
        ] {
            assert_eq!(parse_frame(message), Err(ProtocolError::InvalidMessage));
        }
    }
    #[test]
    fn accepts_an_uncorrelated_timer_state_update() {
        assert!(parse_frame(br#"{"version":1,"type":"state","message_id":"a","health":"active","session_id":"session","timer_state":"paused","remaining_seconds":60}"#).is_ok());
    }
}
