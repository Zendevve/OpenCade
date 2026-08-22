//! OpenFight protocol — versioned envelope for REST + WebSocket messages.
//! Keep in sync with `apps/server/src/main.rs` Envelope stub.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Versioned envelope wrapping every protocol message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
    pub payload: T,
}

impl<T> Envelope<T> {
    pub fn new(msg_type: impl Into<String>, payload: T) -> Self {
        Self {
            msg_type: msg_type.into(),
            version: "1.0".to_string(),
            request_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            payload,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip() {
        let env = Envelope::new("health.ok", serde_json::json!({ "status": "ok" }));
        let s = serde_json::to_string(&env).unwrap();
        let back: Envelope = serde_json::from_str(&s).unwrap();
        assert_eq!(back.msg_type, "health.ok");
        assert_eq!(back.version, "1.0");
    }
}
