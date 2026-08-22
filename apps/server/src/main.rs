use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tracing::info;
use uuid::Uuid;

// Versioned envelope shared with packages/protocol.
// Keep in sync with openfight-protocol::Envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope<T = serde_json::Value> {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub version: String,
    pub request_id: String,
    pub timestamp: String,
    pub payload: T,
}

impl<T: Serialize> Envelope<T> {
    pub fn new(msg_type: impl Into<String>, payload: T) -> Self {
        Self {
            msg_type: msg_type.into(),
            version: "1.0".to_string(),
            request_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            payload,
        }
    }
}

async fn health() -> impl IntoResponse {
    let env = Envelope::new("health.ok", serde_json::json!({ "status": "ok" }));
    (StatusCode::OK, Json(env))
}

async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(mut socket: WebSocket) {
    // Send hello envelope on connect
    let hello = Envelope::new("connection.hello", serde_json::json!({ "message": "connected" }));
    if let Ok(text) = serde_json::to_string(&hello) {
        let _ = socket.send(Message::Text(text)).await;
    }

    // Echo loop: wrap incoming text in versioned envelope
    while let Some(Ok(msg)) = socket.recv().await {
        match msg {
            Message::Text(text) => {
                let incoming: Result<serde_json::Value, _> = serde_json::from_str(&text);
                let payload = match incoming {
                    Ok(v) => v,
                    Err(_) => serde_json::Value::String(text),
                };
                let echo = Envelope::new("echo", payload);
                if let Ok(out) = serde_json::to_string(&echo) {
                    if socket.send(Message::Text(out)).await.is_err() {
                        break;
                    }
                }
            }
            Message::Binary(bin) => {
                let echo = Envelope::new(
                    "echo.binary",
                    serde_json::json!({ "bytes": bin.len() }),
                );
                if let Ok(out) = serde_json::to_string(&echo) {
                    let _ = socket.send(Message::Text(out)).await;
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let app = Router::new()
        .route("/health", get(health))
        .route("/ws", get(ws_handler));

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("openfight-server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
