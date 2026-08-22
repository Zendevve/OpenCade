//! WebSocket upgrade and message handling.
//!
//! The protocol is versioned. Every message on the wire is an
//! [`Envelope`] with `{ type, version, request_id, timestamp, payload }`.
//! Clients MUST send `version: "1"` or `version: "1.0"` (alias for
//! [`PROTOCOL_VERSION`]). Unknown versions receive an error envelope
//! with code `version_unsupported`. Unknown `type` values receive an
//! error envelope with code `unknown_type`.

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use openfight_protocol::{Envelope, PROTOCOL_VERSION};
use serde_json::{json, Value};
use tracing::info;

use crate::state::AppState;

/// Axum handler for `GET /ws` — upgrades the connection.
pub async fn ws_handler(State(state): State<AppState>, ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Core WebSocket loop.
///
/// Sends a `connection.hello` envelope immediately after upgrade, then
/// processes incoming `Text` frames as versioned envelopes. `Binary`,
/// `Ping` / `Pong` and `Close` are handled explicitly. No `unwrap` is
/// used — every fallible operation is matched and logged.
pub async fn handle_socket(mut socket: WebSocket, state: AppState) {
    // Cheap tracing that the hub is reachable; prevents unused warning
    // and is useful for ops dashboards.
    info!(
        ws_hub_size = state.ws_hub.len(),
        "ws: new connection, sending hello"
    );

    // 1. Send hello envelope.
    let hello = Envelope::new(
        "connection.hello",
        json!({ "message": "connected", "protocol_version": PROTOCOL_VERSION }),
    );
    match serde_json::to_string(&hello) {
        Ok(text) => {
            if let Err(e) = socket.send(Message::Text(text)).await {
                info!("ws: failed to send hello: {}", e);
                return;
            }
        }
        Err(e) => {
            info!("ws: failed to serialize hello: {}", e);
            return;
        }
    }

    info!("ws: hello sent, entering recv loop");

    // 2. Receive loop.
    while let Some(result) = socket.recv().await {
        let msg = match result {
            Ok(m) => m,
            Err(e) => {
                info!("ws: recv error: {}", e);
                break;
            }
        };

        match msg {
            Message::Text(text) => {
                info!("ws: received text ({} bytes)", text.len());

                // Parse as generic Envelope<Value>.
                let incoming: Result<Envelope<Value>, serde_json::Error> =
                    serde_json::from_str(&text);

                let envelope = match incoming {
                    Ok(env) => env,
                    Err(e) => {
                        info!("ws: invalid json: {}", e);
                        let err_env = Envelope::new(
                            "error.bad_request",
                            json!({
                                "code": "bad_request",
                                "message": format!("invalid json: {}", e),
                            }),
                        );
                        if let Ok(out) = serde_json::to_string(&err_env) {
                            if socket.send(Message::Text(out)).await.is_err() {
                                break;
                            }
                        }
                        continue;
                    }
                };

                // Validate version — accept PROTOCOL_VERSION, "1", "1.0".
                if !is_supported_version(&envelope.version) {
                    info!(
                        "ws: unsupported version {} (expected {} or 1)",
                        envelope.version, PROTOCOL_VERSION
                    );
                    let err_env = Envelope::new(
                        "error.version_unsupported",
                        json!({
                            "code": "version_unsupported",
                            "message": format!(
                                "unsupported protocol version: {} (expected {} or 1)",
                                envelope.version, PROTOCOL_VERSION
                            ),
                            "received_version": envelope.version,
                        }),
                    );
                    if let Ok(out) = serde_json::to_string(&err_env) {
                        if socket.send(Message::Text(out)).await.is_err() {
                            break;
                        }
                    }
                    continue;
                }

                // Validate msg_type is non-empty (after trimming).
                if envelope.msg_type.trim().is_empty() {
                    info!("ws: empty msg_type");
                    let err_env = Envelope::new(
                        "error.bad_request",
                        json!({
                            "code": "bad_request",
                            "message": "msg_type must not be empty",
                        }),
                    );
                    if let Ok(out) = serde_json::to_string(&err_env) {
                        if socket.send(Message::Text(out)).await.is_err() {
                            break;
                        }
                    }
                    continue;
                }

                // Dispatch on msg_type.
                match envelope.msg_type.as_str() {
                    // Known echo / ping types — reflect payload back.
                    "connection.echo" | "connection.ping" | "ping" | "echo" => {
                        info!("ws: echo for type {}", envelope.msg_type);
                        let echo = Envelope::new("connection.echo", envelope.payload);
                        match serde_json::to_string(&echo) {
                            Ok(out) => {
                                if socket.send(Message::Text(out)).await.is_err() {
                                    info!("ws: echo send failed, closing");
                                    break;
                                }
                            }
                            Err(e) => {
                                info!("ws: failed to serialize echo: {}", e);
                            }
                        }
                    }
                    // Unknown type — return error envelope.
                    other => {
                        info!("ws: unknown msg_type: {}", other);
                        let err_env = Envelope::new(
                            "error.unknown_type",
                            json!({
                                "code": "unknown_type",
                                "message": format!("unknown message type: {}", other),
                                "received_type": other,
                            }),
                        );
                        match serde_json::to_string(&err_env) {
                            Ok(out) => {
                                if socket.send(Message::Text(out)).await.is_err() {
                                    break;
                                }
                            }
                            Err(e) => {
                                info!("ws: failed to serialize error envelope: {}", e);
                            }
                        }
                    }
                }
            }
            Message::Binary(bin) => {
                info!("ws: received binary ({} bytes)", bin.len());
                let echo = Envelope::new(
                    "connection.echo",
                    json!({ "bytes": bin.len(), "protocol_version": PROTOCOL_VERSION }),
                );
                match serde_json::to_string(&echo) {
                    Ok(out) => {
                        if socket.send(Message::Text(out)).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        info!("ws: failed to serialize binary echo: {}", e);
                    }
                }
            }
            Message::Ping(data) => {
                info!("ws: ping ({} bytes), replying pong", data.len());
                if socket.send(Message::Pong(data)).await.is_err() {
                    break;
                }
            }
            Message::Pong(_) => {
                info!("ws: pong received");
            }
            Message::Close(frame) => {
                if let Some(f) = frame {
                    info!("ws: close frame code={} reason={}", f.code, f.reason);
                } else {
                    info!("ws: close frame without payload");
                }
                break;
            }
        }
    }

    info!("ws: connection closed");
}

/// Return true if `version` is accepted by the server.
///
/// Accepted values:
/// - [`PROTOCOL_VERSION`] (currently `"1.0"`)
/// - `"1"`
/// - `"1.0"` (alias, identical to `PROTOCOL_VERSION` today)
fn is_supported_version(version: &str) -> bool {
    version == PROTOCOL_VERSION || version == "1" || version == "1.0"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_versions() {
        assert!(is_supported_version("1.0"));
        assert!(is_supported_version("1"));
        assert!(is_supported_version(PROTOCOL_VERSION));
        assert!(!is_supported_version("2.0"));
        assert!(!is_supported_version(""));
        assert!(!is_supported_version("1.1"));
    }

    #[test]
    fn hello_serialization() {
        let hello = Envelope::new(
            "connection.hello",
            json!({ "message": "connected", "protocol_version": PROTOCOL_VERSION }),
        );
        let s = serde_json::to_string(&hello).expect("serialize hello");
        assert!(s.contains("connection.hello"));
        assert!(s.contains(PROTOCOL_VERSION));
    }
}
