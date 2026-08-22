//! OpenFight protocol — versioned envelope for REST + WebSocket messages.
//! Keep in sync with `apps/server/src/main.rs` Envelope stub.
//!
//! # Wire format
//! Every message is an [`Envelope`] carrying `{ type, version, request_id, timestamp, payload }`.
//! Version `1.0` is canonical; `"1"` is accepted for backwards compatibility.
//! Payloads are strongly typed structs serialized with `snake_case` keys.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// Canonical protocol version. All messages SHOULD use this value.
pub const PROTOCOL_VERSION: &str = "1.0";

/// Returns `true` if the supplied version string is supported.
///
/// Accepts both canonical `"1.0"` and compat `"1"`.
pub fn is_supported_version(v: &str) -> bool {
    v == "1.0" || v == "1"
}
/// Versioned envelope wrapping every protocol message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/generated/Envelope.ts", concrete(T = String))]
pub struct Envelope<T = serde_json::Value> {
    /// Discriminator for message kind, e.g. `lobby.create`, `challenge.offer`.
    #[serde(rename = "type")]
    pub msg_type: String,
    /// Protocol version, e.g. "1.0".
    pub version: String,
    /// Unique id for request/response correlation.
    pub request_id: String,
    /// RFC 3339 timestamp (UTC).
    pub timestamp: DateTime<Utc>,
    /// Arbitrary payload.
    #[ts(type = "unknown")]
    pub payload: T,
}

impl<T> Envelope<T> {
    /// Create a new envelope with the canonical version, a fresh v4 UUID and `Utc::now()`.
    pub fn new(msg_type: impl Into<String>, payload: T) -> Self {
        Self {
            msg_type: msg_type.into(),
            version: PROTOCOL_VERSION.to_string(),
            request_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            payload,
        }
    }

    /// Validate envelope fields.
    ///
    /// * `version` must be supported (`"1.0"` or `"1"`).
    /// * `msg_type` must be non-empty (after trimming).
    pub fn validate(&self) -> Result<(), String> {
        if !is_supported_version(&self.version) {
            return Err(format!("unsupported version: {}", self.version));
        }
        if self.msg_type.trim().is_empty() {
            return Err("msg_type must not be empty".to_string());
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Payloads
// ---------------------------------------------------------------------------
/// Presence / latency update. Mirrors `diagnose_network` and `presence.update`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export, export_to = "src/generated/PresencePayload.ts")]
#[serde(rename_all = "snake_case")]
pub struct PresencePayload {
    pub user_id: String,
    pub rtt_ms: u32,
    pub loss: f32,
    pub jitter: u32,
    pub relay_reachable: bool,
}
/// Chat message. Used for `chat.message` in rooms/lobbies.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/generated/ChatPayload.ts")]
#[serde(rename_all = "snake_case")]
pub struct ChatPayload {
    pub channel: String,
    pub body: String,
    pub author_id: String,
}
/// Challenge request between two users for a game/room.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/generated/ChallengePayload.ts")]
#[serde(rename_all = "snake_case")]
pub struct ChallengePayload {
    pub room_id: String,
    pub game_id: String,
    pub challenger_id: String,
    pub challenged_id: String,
}
/// WebRTC / signaling payload: offer/answer/candidate relayed via server.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/generated/SessionPayload.ts")]
#[serde(rename_all = "snake_case")]
pub struct SessionPayload {
    pub room_id: String,
    pub sdp_type: String,
    pub sdp: String,
    pub candidate: String,
}
/// Room lifecycle states. Serialized as `snake_case` strings.
/// ARCHITECTURE.md §9 DB stores WAITING/READY/PLAYING/FINISHED/CANCELLED (upper);
/// AGENTS.md describes WAITING→CHALLENGING→CONNECTING→PLAYING→FINISHED|CANCELLED.
/// Rust payload is snake_case lowercase for the wire; `Ready` is kept for compat with ARCH's READY.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/generated/RoomState.ts")]
#[serde(rename_all = "snake_case")]
pub enum RoomState {
    Waiting,
    Ready,
    Challenging,
    Connecting,
    Playing,
    Finished,
    Cancelled,
}
/// Room snapshot pushed via `room.state`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/generated/RoomPayload.ts")]
#[serde(rename_all = "snake_case")]
pub struct RoomPayload {
    pub id: String,
    pub game_id: String,
    pub host_id: String,
    pub guest_id: Option<String>,
    pub state: RoomState,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn roundtrip() {
        let env = Envelope::new("health.ok", json!({ "status": "ok" }));
        let s = serde_json::to_string(&env).unwrap();
        let back: Envelope = serde_json::from_str(&s).unwrap();
        assert_eq!(back.msg_type, "health.ok");
        assert_eq!(back.version, "1.0");
    }

    #[test]
    fn roundtrip_presence() {
        let payload = PresencePayload {
            user_id: "user-1".to_string(),
            rtt_ms: 42,
            loss: 0.02,
            jitter: 5,
            relay_reachable: true,
        };
        let env = Envelope::new("presence.update", payload.clone());
        let s = serde_json::to_string(&env).unwrap();
        let back: Envelope<PresencePayload> = serde_json::from_str(&s).unwrap();
        assert_eq!(back.payload, payload);
        assert_eq!(back.version, "1.0");
        assert!(back.validate().is_ok());
    }

    #[test]
    fn roundtrip_chat() {
        let payload = ChatPayload {
            channel: "lobby:1".to_string(),
            body: "hello".to_string(),
            author_id: "user-1".to_string(),
        };
        let env = Envelope::new("chat.message", payload.clone());
        let s = serde_json::to_string(&env).unwrap();
        let back: Envelope<ChatPayload> = serde_json::from_str(&s).unwrap();
        assert_eq!(back.payload, payload);
    }

    #[test]
    fn roundtrip_challenge() {
        let payload = ChallengePayload {
            room_id: "room-1".to_string(),
            game_id: "kof98".to_string(),
            challenger_id: "user-1".to_string(),
            challenged_id: "user-2".to_string(),
        };
        let env = Envelope::new("challenge.create", payload.clone());
        let s = serde_json::to_string(&env).unwrap();
        let back: Envelope<ChallengePayload> = serde_json::from_str(&s).unwrap();
        assert_eq!(back.payload, payload);
    }

    #[test]
    fn roundtrip_session() {
        let payload = SessionPayload {
            room_id: "room-1".to_string(),
            sdp_type: "offer".to_string(),
            sdp: "v=0\r\n...".to_string(),
            candidate: "candidate:1 ...".to_string(),
        };
        let env = Envelope::new("signaling.offer", payload.clone());
        let s = serde_json::to_string(&env).unwrap();
        let back: Envelope<SessionPayload> = serde_json::from_str(&s).unwrap();
        assert_eq!(back.payload, payload);
    }

    #[test]
    fn roundtrip_room() {
        let payload = RoomPayload {
            id: "room-1".to_string(),
            game_id: "kof98".to_string(),
            host_id: "user-1".to_string(),
            guest_id: Some("user-2".to_string()),
            state: RoomState::Waiting,
        };
        let env = Envelope::new("room.state", payload.clone());
        let s = serde_json::to_string(&env).unwrap();
        let back: Envelope<RoomPayload> = serde_json::from_str(&s).unwrap();
        assert_eq!(back.payload, payload);
    }

    #[test]
    fn room_state_variants_serde() {
        let cases = vec![
            (RoomState::Waiting, "waiting"),
            (RoomState::Ready, "ready"),
            (RoomState::Challenging, "challenging"),
            (RoomState::Connecting, "connecting"),
            (RoomState::Playing, "playing"),
            (RoomState::Finished, "finished"),
            (RoomState::Cancelled, "cancelled"),
        ];
        for (state, expected) in cases {
            let s = serde_json::to_string(&state).unwrap();
            assert_eq!(s, format!("\"{}\"", expected));
            let back: RoomState = serde_json::from_str(&s).unwrap();
            assert_eq!(back, state);
        }
    }

    #[test]
    fn room_payload_optional_guest() {
        let payload = RoomPayload {
            id: "room-2".to_string(),
            game_id: "sfiii3".to_string(),
            host_id: "user-1".to_string(),
            guest_id: None,
            state: RoomState::Waiting,
        };
        let s = serde_json::to_string(&payload).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert!(v.get("guest_id").is_some());
        let back: RoomPayload = serde_json::from_str(&s).unwrap();
        assert_eq!(back.guest_id, None);
    }

    #[test]
    fn type_field_renamed() {
        let env = Envelope::new("presence.update", json!({ "x": 1 }));
        let s = serde_json::to_string(&env).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert!(
            v.get("type").is_some(),
            "envelope must serialize msg_type as \"type\""
        );
        assert!(
            v.get("msg_type").is_none(),
            "envelope must not expose \"msg_type\" key"
        );
        assert_eq!(v["type"], "presence.update");
        // roundtrip rename
        let back: Envelope = serde_json::from_str(&s).unwrap();
        assert_eq!(back.msg_type, "presence.update");
    }

    #[test]
    fn snake_case_payload_keys() {
        let payload = PresencePayload {
            user_id: "user-1".to_string(),
            rtt_ms: 42,
            loss: 0.5,
            jitter: 7,
            relay_reachable: false,
        };
        let s = serde_json::to_string(&payload).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert!(v.get("rtt_ms").is_some(), "expected snake_case rtt_ms");
        assert!(v.get("user_id").is_some());
        assert!(v.get("relay_reachable").is_some());
        assert!(v.get("rttMs").is_none(), "must not be camelCase");
        assert!(v.get("relayReachable").is_none());
        // also check raw string contains snake_case
        assert!(s.contains("rtt_ms"));
        assert!(!s.contains("rttMs"));
    }

    #[test]
    fn snake_case_chat_keys() {
        let payload = ChatPayload {
            channel: "general".to_string(),
            body: "hi".to_string(),
            author_id: "user-1".to_string(),
        };
        let s = serde_json::to_string(&payload).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert!(v.get("author_id").is_some());
        assert!(v.get("authorId").is_none());
    }

    #[test]
    fn version_check() {
        assert_eq!(PROTOCOL_VERSION, "1.0");
        assert!(is_supported_version("1.0"));
        assert!(is_supported_version("1"));
        assert!(!is_supported_version("2.0"));
        assert!(!is_supported_version(""));
        assert!(!is_supported_version("1.1"));
    }

    #[test]
    fn validate_ok_and_err() {
        let mut env = Envelope::new("test.event", json!({}));
        assert!(env.validate().is_ok());

        env.version = "2.0".to_string();
        assert!(env.validate().is_err(), "unsupported version should fail");

        env.version = "1".to_string();
        assert!(
            env.validate().is_ok(),
            "\"1\" should be accepted for compat"
        );

        env.version = "1.0".to_string();
        env.msg_type = "".to_string();
        assert!(env.validate().is_err(), "empty msg_type should fail");

        env.msg_type = "   ".to_string();
        assert!(
            env.validate().is_err(),
            "whitespace-only msg_type should fail"
        );

        env.msg_type = "valid.type".to_string();
        assert!(env.validate().is_ok());
    }

    #[test]
    fn envelope_new_sets_expected_fields() {
        let env = Envelope::new("my.type", json!({ "a": 1 }));
        assert_eq!(env.msg_type, "my.type");
        assert_eq!(env.version, "1.0");
        assert!(!env.request_id.is_empty());
        // request_id should be valid UUID v4
        assert!(uuid::Uuid::parse_str(&env.request_id).is_ok());
    }
}
