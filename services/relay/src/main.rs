use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::{collections::HashMap, env, net::SocketAddr, sync::Arc};
use tokio::sync::mpsc;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

use opencade_protocol::{is_supported_version, Envelope};

#[derive(Debug, Clone)]
struct Config {
    port: u16,
    relay_port: u16,
    relay_host: Option<String>,
    rust_log: String,
}

impl Config {
    fn from_env() -> Self {
        let relay_port = env::var("RELAY_PORT")
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(3478);
        let port = env::var("PORT")
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(8081);
        let rust_log = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
        let relay_host = env::var("RELAY_HOST").ok().filter(|s| !s.trim().is_empty());
        Self {
            port,
            relay_port,
            relay_host,
            rust_log,
        }
    }
}

#[derive(Debug)]
struct RelayState {
    config: Config,
    rooms: DashMap<String, DashMap<Uuid, mpsc::UnboundedSender<Message>>>,
}

type SharedState = Arc<RelayState>;

fn init_tracing(rust_log: &str) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(rust_log));
    let is_prod = env::var("OPENCADE_ENV")
        .map(|v| v.eq_ignore_ascii_case("production"))
        .unwrap_or(false);
    if is_prod {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(false)
            .json()
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(false)
            .pretty()
            .init();
    }
}

async fn health(State(state): State<SharedState>) -> impl IntoResponse {
    let body = json!({
        "status": "ok",
        "version": "0.1.0",
        "relay_port": state.config.relay_port,
    });
    (StatusCode::OK, Json(body))
}

async fn ready() -> impl IntoResponse {
    let body = json!({ "status": "ok" });
    (StatusCode::OK, Json(body))
}

async fn relay_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let room_hint = params
        .get("room_id")
        .cloned()
        .unwrap_or_else(|| "default".to_string());
    info!(room_hint = %room_hint, params = ?params, "relay_ws upgrade");
    ws.on_upgrade(move |socket| handle_socket(socket, state, room_hint))
}

fn extract_room_id_from_text(text: &str, fallback: &str) -> String {
    if let Ok(value) = serde_json::from_str::<Value>(text) {
        if let Some(payload) = value.get("payload") {
            if let Some(room) = payload.get("room_id").and_then(|v| v.as_str()) {
                if !room.trim().is_empty() {
                    return room.to_string();
                }
            }
        }
        if let Some(room) = value.get("room_id").and_then(|v| v.as_str()) {
            if !room.trim().is_empty() {
                return room.to_string();
            }
        }
    }
    fallback.to_string()
}

fn broadcast_message(state: &SharedState, room_id: &str, msg: Message, skip_id: Uuid) -> usize {
    let mut sent = 0usize;
    if let Some(room) = state.rooms.get(room_id) {
        for entry in room.iter() {
            if *entry.key() == skip_id {
                continue;
            }
            let _ = entry.value().send(msg.clone());
            sent += 1;
        }
    }
    sent
}

async fn handle_socket(socket: WebSocket, state: SharedState, initial_room: String) {
    let id = Uuid::new_v4();
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    {
        let room = state.rooms.entry(initial_room.clone()).or_default();
        room.insert(id, tx.clone());
    }
    info!(peer_id = %id, room_id = %initial_room, "relay peer connected");
    info!(
        peer_id = %id,
        room_id = %initial_room,
        peers_in_room = state.rooms.get(&initial_room).map(|r| r.len()).unwrap_or(0),
        "peer inserted"
    );

    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    while let Some(result) = ws_receiver.next().await {
        match result {
            Ok(msg) => match msg {
                Message::Text(text) => {
                    let text_str = text.to_string();
                    info!(peer_id = %id, room_id = %initial_room, text = %text_str, "recv text");
                    if let Ok(envelope) = serde_json::from_str::<Envelope<Value>>(&text_str) {
                        if !is_supported_version(&envelope.version) {
                            warn!(peer_id = %id, version = %envelope.version, "unsupported version");
                            let err = json!({
                                "type": "error",
                                "code": "unsupported_version",
                                "message": format!("unsupported version: {}", envelope.version),
                                "request_id": envelope.request_id,
                            });
                            let _ = tx.send(Message::Text(err.to_string()));
                            continue;
                        }
                        if let Err(e) = envelope.validate() {
                            warn!(peer_id = %id, error = %e, "envelope validation failed");
                            let err = json!({
                                "type": "error",
                                "code": "invalid_envelope",
                                "message": e,
                                "request_id": envelope.request_id,
                            });
                            let _ = tx.send(Message::Text(err.to_string()));
                            continue;
                        }
                        let target_room = envelope
                            .payload
                            .get("room_id")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| initial_room.clone());
                        if target_room != initial_room {
                            let room = state.rooms.entry(target_room.clone()).or_default();
                            room.insert(id, tx.clone());
                        }
                        let sent = broadcast_message(
                            &state,
                            &target_room,
                            Message::Text(text.clone()),
                            id,
                        );
                        info!(peer_id = %id, target_room = %target_room, sent = sent, "broadcast envelope");
                    } else {
                        let target_room = extract_room_id_from_text(&text_str, &initial_room);
                        if target_room != initial_room {
                            let room = state.rooms.entry(target_room.clone()).or_default();
                            room.insert(id, tx.clone());
                        }
                        let sent = broadcast_message(
                            &state,
                            &target_room,
                            Message::Text(text.clone()),
                            id,
                        );
                        info!(peer_id = %id, target_room = %target_room, sent = sent, "broadcast opaque");
                    }
                }
                Message::Binary(bin) => {
                    let sent =
                        broadcast_message(&state, &initial_room, Message::Binary(bin.clone()), id);
                    info!(peer_id = %id, sent = sent, "broadcast binary");
                }
                Message::Ping(payload) => {
                    let _ = tx.send(Message::Pong(payload));
                }
                Message::Pong(_) => {}
                Message::Close(frame) => {
                    if let Some(f) = frame {
                        info!(peer_id = %id, code = %f.code, reason = %f.reason, "close frame");
                    }
                    break;
                }
            },
            Err(e) => {
                warn!(peer_id = %id, error = %e, "websocket receive error");
                break;
            }
        }
    }

    for entry in state.rooms.iter() {
        entry.remove(&id);
    }
    let empty_rooms: Vec<String> = state
        .rooms
        .iter()
        .filter(|e| e.is_empty())
        .map(|e| e.key().clone())
        .collect();
    for room_id in empty_rooms {
        state.rooms.remove(&room_id);
    }

    send_task.abort();
    info!(peer_id = %id, "relay peer disconnected");
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    info!("shutdown signal received");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env();
    init_tracing(&config.rust_log);

    if let Some(host) = &config.relay_host {
        info!(%host, relay_port = config.relay_port, "relay host configured");
    }
    info!(
        port = config.port,
        relay_port = config.relay_port,
        "opencade-relay starting"
    );

    let state = Arc::new(RelayState {
        config: config.clone(),
        rooms: DashMap::new(),
    });

    let app = Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/relay", get(relay_ws_handler))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(%addr, "opencade-relay listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}
