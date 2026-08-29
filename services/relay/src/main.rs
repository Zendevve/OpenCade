use axum::{
    Json, Router,
    extract::{
        Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use opencade_protocol::{BorrowedEnvelope, is_supported_version};
use opencade_shared::{RelayCapability, RelayTicket};
use serde::Deserialize;
use serde_json::json;
use std::{
    env,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{mpsc, watch};
use tracing::{debug, info, warn};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

#[derive(Deserialize)]
struct RelayPayloadRef<'a> {
    #[serde(borrow)]
    room_id: &'a str,
}

#[derive(Clone)]
struct Config {
    port: u16,
    rust_log: String,
    auth_secret: String,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Config")
            .field("port", &self.port)
            .field("rust_log", &self.rust_log)
            .field("auth_secret", &"<redacted>")
            .finish()
    }
}

impl Config {
    fn from_env() -> Result<Self, String> {
        let port = match env::var("PORT") {
            Ok(value) => value
                .parse::<u16>()
                .map_err(|_| "PORT must be a valid TCP port".to_string())?,
            Err(env::VarError::NotPresent) => 8081,
            Err(env::VarError::NotUnicode(_)) => return Err("PORT must be valid Unicode".into()),
        };
        let rust_log = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
        let auth_secret = env::var("RELAY_AUTH_SECRET")
            .map_err(|_| "RELAY_AUTH_SECRET is required".to_string())?;
        if auth_secret.len() < 32 {
            return Err("RELAY_AUTH_SECRET must contain at least 32 characters".into());
        }
        Ok(Self {
            port,
            rust_log,
            auth_secret,
        })
    }
}

#[derive(Debug)]
struct RelayState {
    config: Config,
    rooms: DashMap<String, DashMap<String, RelayPeer>>,
    used_tickets: DashMap<Uuid, i64>,
}

#[derive(Debug, Clone)]
struct RelayPeer {
    connection_id: Uuid,
    sender: mpsc::Sender<Message>,
    cancel: watch::Sender<()>,
}

type SharedState = Arc<RelayState>;

fn build_relay_app(state: SharedState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/relay", get(relay_ws_handler))
        .with_state(state)
}

fn spawn_ticket_cleanup(state: SharedState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .ok()
                .and_then(|duration| i64::try_from(duration.as_secs()).ok());
            if let Some(now) = now {
                state
                    .used_tickets
                    .retain(|_, expires_at| *expires_at >= now);
            }
        }
    });
}

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

async fn health() -> impl IntoResponse {
    let body = json!({
        "status": "ok",
        "version": "0.1.0",
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
    Query(ticket): Query<RelayTicket>,
) -> axum::response::Response {
    let now = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => i64::try_from(duration.as_secs()).unwrap_or(i64::MAX),
        Err(_) => {
            return (StatusCode::SERVICE_UNAVAILABLE, "system clock unavailable").into_response();
        }
    };
    if ticket
        .verify(state.config.auth_secret.as_bytes(), now)
        .is_err()
    {
        return (StatusCode::UNAUTHORIZED, "invalid or expired relay ticket").into_response();
    }
    if state
        .used_tickets
        .insert(ticket.nonce, ticket.expires_at)
        .is_some()
    {
        return (
            StatusCode::UNAUTHORIZED,
            "relay ticket has already been used",
        )
            .into_response();
    }
    let room_id = ticket.room_id;
    let user_id = ticket.user_id;
    let capability = ticket.capability;
    let expires_at = ticket.expires_at;
    info!(room_id = %room_id, "authorized relay upgrade");
    ws.on_upgrade(move |socket| {
        handle_socket(socket, state, room_id, user_id, capability, expires_at)
    })
    .into_response()
}

fn broadcast_message(state: &SharedState, room_id: &str, msg: Message, skip_id: Uuid) -> usize {
    let mut sent = 0usize;
    if let Some(room) = state.rooms.get(room_id) {
        for entry in room.iter() {
            if entry.value().connection_id == skip_id {
                continue;
            }
            if entry.value().sender.try_send(msg.clone()).is_ok() {
                sent += 1;
            }
        }
    }
    sent
}

async fn handle_socket(
    socket: WebSocket,
    state: SharedState,
    room_id: String,
    user_id: String,
    capability: RelayCapability,
    expires_at: i64,
) {
    let id = Uuid::new_v4();
    let room_key = format!("{room_id}:{}", capability.as_str());
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (tx, mut rx) = mpsc::channel::<Message>(64);
    let (cancel_tx, mut cancel_rx) = watch::channel(());

    {
        let room = state.rooms.entry(room_key.clone()).or_default();
        if room.len() >= 2 && !room.contains_key(&user_id) {
            warn!(room_id = %room_id, "relay room is full");
            return;
        }
        let previous = room.insert(
            user_id.clone(),
            RelayPeer {
                connection_id: id,
                sender: tx.clone(),
                cancel: cancel_tx,
            },
        );
        if let Some(previous) = previous {
            let _ = previous.cancel.send(());
            let _ = previous.sender.try_send(Message::Close(None));
        }
    }
    info!(peer_id = %id, room_id = %room_id, "relay peer connected");
    info!(
        peer_id = %id,
        room_id = %room_id,
        peers_in_room = state.rooms.get(&room_key).map(|r| r.len()).unwrap_or(0),
        "peer inserted"
    );

    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or(expires_at);
    let lifetime = Duration::from_secs(u64::try_from(expires_at.saturating_sub(now)).unwrap_or(0));
    let expiry = tokio::time::sleep(lifetime);
    tokio::pin!(expiry);
    let mut window_started = Instant::now();
    let mut window_messages = 0_u32;
    let mut window_bytes = 0_usize;

    loop {
        let result = tokio::select! {
            biased;
            _ = cancel_rx.changed() => break,
            _ = &mut expiry => {
                debug!(peer_id = %id, room_id = %room_id, "relay ticket lifetime elapsed");
                break;
            }
            result = ws_receiver.next() => result,
        };
        let Some(result) = result else { break };
        match result {
            Ok(msg) => match msg {
                Message::Text(text) => {
                    if exceeds_rate_limit(
                        &mut window_started,
                        &mut window_messages,
                        &mut window_bytes,
                        text.len(),
                    ) {
                        warn!(peer_id = %id, room_id = %room_id, "relay rate limit exceeded");
                        break;
                    }
                    if text.len() > 64 * 1024 {
                        warn!(peer_id = %id, room_id = %room_id, "oversized relay text rejected");
                        continue;
                    }
                    if let Ok(envelope) = serde_json::from_str::<BorrowedEnvelope<'_>>(&text) {
                        if !is_supported_version(envelope.version) {
                            warn!(peer_id = %id, version = %envelope.version, "unsupported version");
                            let err = json!({
                                "type": "error",
                                "code": "unsupported_version",
                                "message": format!("unsupported version: {}", envelope.version),
                                "request_id": envelope.request_id,
                            });
                            let _ = tx.try_send(Message::Text(err.to_string().into()));
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
                            let _ = tx.try_send(Message::Text(err.to_string().into()));
                            continue;
                        }
                        let envelope_room = envelope
                            .payload_as::<RelayPayloadRef<'_>>()
                            .map(|payload| payload.room_id)
                            .unwrap_or_default();
                        if envelope_room != room_id {
                            warn!(peer_id = %id, room_id = %room_id, "cross-room relay rejected");
                            continue;
                        }
                        let sent =
                            broadcast_message(&state, &room_key, Message::Text(text.clone()), id);
                        debug!(peer_id = %id, room_id = %room_id, sent = sent, "broadcast envelope");
                    } else {
                        warn!(peer_id = %id, room_id = %room_id, "non-envelope relay text rejected");
                    }
                }
                Message::Binary(bin) => {
                    if exceeds_rate_limit(
                        &mut window_started,
                        &mut window_messages,
                        &mut window_bytes,
                        bin.len(),
                    ) {
                        warn!(peer_id = %id, room_id = %room_id, "relay rate limit exceeded");
                        break;
                    }
                    let maximum = match capability {
                        RelayCapability::Probe => 64 * 1024,
                        RelayCapability::NativeTcpTunnel => 16 * 1024,
                    };
                    if bin.len() > maximum {
                        warn!(peer_id = %id, room_id = %room_id, "oversized relay frame rejected");
                        continue;
                    }
                    let sent =
                        broadcast_message(&state, &room_key, Message::Binary(bin.clone()), id);
                    debug!(peer_id = %id, sent = sent, "broadcast binary");
                }
                Message::Ping(payload) => {
                    let _ = tx.try_send(Message::Pong(payload));
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

    if let Some(room) = state.rooms.get(&room_key)
        && room
            .get(&user_id)
            .is_some_and(|peer| peer.connection_id == id)
    {
        room.remove(&user_id);
    }
    state.rooms.remove_if(&room_key, |_, room| room.is_empty());

    send_task.abort();
    info!(peer_id = %id, "relay peer disconnected");
}

fn exceeds_rate_limit(
    window_started: &mut Instant,
    messages: &mut u32,
    bytes: &mut usize,
    frame_bytes: usize,
) -> bool {
    const MAX_MESSAGES_PER_SECOND: u32 = 128;
    const MAX_BYTES_PER_SECOND: usize = 1024 * 1024;
    if window_started.elapsed() >= Duration::from_secs(1) {
        *window_started = Instant::now();
        *messages = 0;
        *bytes = 0;
    }
    *messages = messages.saturating_add(1);
    *bytes = bytes.saturating_add(frame_bytes);
    *messages > MAX_MESSAGES_PER_SECOND || *bytes > MAX_BYTES_PER_SECOND
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
    let config = Config::from_env()?;
    init_tracing(&config.rust_log);

    info!(port = config.port, "opencade-relay starting");

    let state = Arc::new(RelayState {
        config: config.clone(),
        rooms: DashMap::new(),
        used_tickets: DashMap::new(),
    });

    spawn_ticket_cleanup(Arc::clone(&state));
    let app = build_relay_app(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(%addr, "opencade-relay listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{SinkExt, StreamExt};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio_tungstenite::{connect_async, tungstenite::Message as ClientMessage};

    const SECRET: &str = "relay-integration-secret-at-least-32-bytes";

    fn state() -> SharedState {
        Arc::new(RelayState {
            config: Config {
                port: 0,
                rust_log: "info".into(),
                auth_secret: SECRET.into(),
            },
            rooms: DashMap::new(),
            used_tickets: DashMap::new(),
        })
    }

    fn ticket_url(address: SocketAddr, ticket: &RelayTicket) -> String {
        format!(
            "ws://{address}/relay?room_id={}&user_id={}&expires_at={}&capability={}&nonce={}&signature={}",
            ticket.room_id,
            ticket.user_id,
            ticket.expires_at,
            ticket.capability.as_str(),
            ticket.nonce,
            ticket.signature
        )
    }

    #[tokio::test]
    async fn signed_room_members_exchange_bounded_binary_frames() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("relay listener");
        let address = listener.local_addr().expect("relay address");
        let server = tokio::spawn(async move {
            axum::serve(listener, build_relay_app(state()))
                .await
                .expect("relay server");
        });
        let now = i64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_secs(),
        )
        .expect("timestamp");
        let host = RelayTicket::issue(SECRET.as_bytes(), "room-1", "host", now + 120)
            .expect("host ticket");
        let guest = RelayTicket::issue(SECRET.as_bytes(), "room-1", "guest", now + 120)
            .expect("guest ticket");
        let (mut host_socket, _) = connect_async(ticket_url(address, &host))
            .await
            .expect("host relay");
        assert!(
            connect_async(ticket_url(address, &host)).await.is_err(),
            "a relay ticket must authorize exactly one upgrade"
        );
        let (mut guest_socket, _) = connect_async(ticket_url(address, &guest))
            .await
            .expect("guest relay");

        host_socket
            .send(ClientMessage::Binary(b"bounded-frame".to_vec().into()))
            .await
            .expect("host send");
        let received = tokio::time::timeout(std::time::Duration::from_secs(1), guest_socket.next())
            .await
            .expect("relay timeout")
            .expect("relay message")
            .expect("relay frame");
        assert_eq!(received.into_data().as_ref(), b"bounded-frame");

        let mut tampered = host;
        tampered.room_id = "other-room".into();
        assert!(connect_async(ticket_url(address, &tampered)).await.is_err());
        server.abort();
    }

    #[test]
    fn byte_and_message_rate_limits_fail_closed() {
        let mut started = Instant::now();
        let mut messages = 127;
        let mut bytes = 0;
        assert!(!exceeds_rate_limit(
            &mut started,
            &mut messages,
            &mut bytes,
            1
        ));
        assert!(exceeds_rate_limit(
            &mut started,
            &mut messages,
            &mut bytes,
            1
        ));

        let mut started = Instant::now();
        let mut messages = 0;
        let mut bytes = 1024 * 1024;
        assert!(exceeds_rate_limit(
            &mut started,
            &mut messages,
            &mut bytes,
            1
        ));
    }

    async fn tcp_pair() -> (tokio::net::TcpStream, tokio::net::TcpStream) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("TCP listener");
        let address = listener.local_addr().expect("TCP address");
        let (client, accepted) =
            tokio::join!(tokio::net::TcpStream::connect(address), listener.accept());
        (
            client.expect("TCP connect"),
            accepted.expect("TCP accept").0,
        )
    }

    #[tokio::test]
    async fn scoped_native_tunnel_carries_bounded_tcp_stream_on_loopback() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("relay listener");
        let address = listener.local_addr().expect("relay address");
        let server = tokio::spawn(async move {
            axum::serve(listener, build_relay_app(state()))
                .await
                .expect("relay server");
        });
        let now = i64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_secs(),
        )
        .expect("timestamp");
        let host_ticket = RelayTicket::issue_scoped(
            SECRET.as_bytes(),
            "native-room",
            "host",
            now + 120,
            RelayCapability::NativeTcpTunnel,
        )
        .expect("host tunnel ticket");
        let guest_ticket = RelayTicket::issue_scoped(
            SECRET.as_bytes(),
            "native-room",
            "guest",
            now + 120,
            RelayCapability::NativeTcpTunnel,
        )
        .expect("guest tunnel ticket");
        let relay_url = format!("ws://{address}/relay");
        let (mut host_app, host_tunnel) = tcp_pair().await;
        let (mut guest_app, guest_tunnel) = tcp_pair().await;
        let host_url = relay_url.clone();
        let host = tokio::spawn(async move {
            opencade_networking::run_native_tcp_tunnel(host_tunnel, &host_url, &host_ticket).await
        });
        let guest = tokio::spawn(async move {
            opencade_networking::run_native_tcp_tunnel(guest_tunnel, &relay_url, &guest_ticket)
                .await
        });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let payload = vec![0x5a; 40 * 1024];
        host_app.write_all(&payload).await.expect("TCP write");
        let mut received = vec![0_u8; payload.len()];
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            guest_app.read_exact(&mut received),
        )
        .await
        .expect("tunnel timeout")
        .expect("TCP read");
        assert_eq!(received, payload);
        host.abort();
        guest.abort();
        server.abort();
    }
}
