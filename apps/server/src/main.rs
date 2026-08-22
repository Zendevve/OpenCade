mod config;
mod error;
mod routes;
mod state;
mod ws;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use dashmap::DashMap;
use openfight_protocol::{Envelope, PROTOCOL_VERSION};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::{env, net::SocketAddr, sync::Arc};
use tokio::sync::mpsc;
use tower_http::{
    cors::{AllowHeaders, AllowMethods, AllowOrigin, CorsLayer},
    trace::TraceLayer,
};
use tracing::info;
use tracing_subscriber::EnvFilter;

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub session_secret: String,
    pub rust_log: String,
    pub port: u16,
}

impl Config {
    pub fn from_env() -> Self {
        let database_url = env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://openfight:openfight@localhost:5432/openfight".to_string());
        let session_secret =
            env::var("SESSION_SECRET").unwrap_or_else(|_| "dev-session-secret-change-me".to_string());
        let rust_log = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
        let port = env::var("PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(8080);
        Self {
            database_url,
            session_secret,
            rust_log,
            port,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::from_env()
    }
}

// ---------------------------------------------------------------------------
// AppState
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Config,
    pub ws_hub: Arc<DashMap<String, mpsc::UnboundedSender<Message>>>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#[allow(dead_code)]
fn envelope_ok(msg_type: &str, payload: Value) -> Envelope<Value> {
    Envelope::new(msg_type, payload)
}

// ---------------------------------------------------------------------------
// Health & Ready
// ---------------------------------------------------------------------------

async fn health_handler() -> (StatusCode, Json<Envelope<Value>>) {
    let payload = json!({
        "status": "ok",
        "version": PROTOCOL_VERSION,
    });
    (StatusCode::OK, Json(Envelope::new("health.ok", payload)))
}

async fn ready_handler(State(state): State<AppState>) -> (StatusCode, Json<Envelope<Value>>) {
    match state.pool.acquire().await {
        Ok(_) => {
            let payload = json!({ "status": "ready", "database": "connected" });
            (StatusCode::OK, Json(Envelope::new("ready.ok", payload)))
        }
        Err(e) => {
            let payload = json!({ "status": "not_ready", "error": e.to_string() });
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(Envelope::new("ready.error", payload)),
            )
        }
    }
}

// ---------------------------------------------------------------------------
// Auth
// ---------------------------------------------------------------------------

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
    pub email: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

async fn register_handler(
    State(_state): State<AppState>,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Envelope<Value>>) {
    // Stub: in production verify username uniqueness, hash password with argon2, insert into DB, issue JWT.
    let username = body
        .get("username")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let payload = json!({
        "message": "user registered",
        "username": username,
        "protocol_version": PROTOCOL_VERSION,
    });
    (StatusCode::CREATED, Json(Envelope::new("auth.register.ok", payload)))
}

async fn login_handler(
    State(_state): State<AppState>,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Envelope<Value>>) {
    let username = body
        .get("username")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    // Stub: verify argon2 hash, issue jsonwebtoken.
    let payload = json!({
        "message": "login successful",
        "username": username,
        "token": "stub-jwt-token",
        "protocol_version": PROTOCOL_VERSION,
    });
    (StatusCode::OK, Json(Envelope::new("auth.login.ok", payload)))
}

async fn logout_handler(State(_state): State<AppState>) -> (StatusCode, Json<Envelope<Value>>) {
    let payload = json!({ "message": "logout successful" });
    (StatusCode::OK, Json(Envelope::new("auth.logout.ok", payload)))
}

// ---------------------------------------------------------------------------
// Games
// ---------------------------------------------------------------------------

async fn list_games_handler(State(_state): State<AppState>) -> (StatusCode, Json<Envelope<Value>>) {
    // Stub: query `games` table.
    let payload = json!({
        "games": [],
        "protocol_version": PROTOCOL_VERSION,
    });
    (StatusCode::OK, Json(Envelope::new("games.list.ok", payload)))
}

async fn get_game_handler(
    State(_state): State<AppState>,
    Path(id): Path<String>,
) -> (StatusCode, Json<Envelope<Value>>) {
    let payload = json!({
        "game": { "id": id, "name": "stub-game", "version": "1.0" },
        "protocol_version": PROTOCOL_VERSION,
    });
    (StatusCode::OK, Json(Envelope::new("games.get.ok", payload)))
}

// ---------------------------------------------------------------------------
// Servers
// ---------------------------------------------------------------------------

async fn list_servers_handler(State(_state): State<AppState>) -> (StatusCode, Json<Envelope<Value>>) {
    let payload = json!({
        "servers": [],
        "protocol_version": PROTOCOL_VERSION,
    });
    (StatusCode::OK, Json(Envelope::new("servers.list.ok", payload)))
}

// ---------------------------------------------------------------------------
// Lobbies
// ---------------------------------------------------------------------------

async fn list_lobbies_handler(
    State(_state): State<AppState>,
    Path(game_id): Path<String>,
) -> (StatusCode, Json<Envelope<Value>>) {
    let payload = json!({
        "game_id": game_id,
        "lobbies": [],
        "protocol_version": PROTOCOL_VERSION,
    });
    (StatusCode::OK, Json(Envelope::new("lobbies.list.ok", payload)))
}

// ---------------------------------------------------------------------------
// Rooms
// ---------------------------------------------------------------------------

#[allow(dead_code)]
#[derive(Debug, Deserialize, Serialize)]
pub struct CreateRoomRequest {
    pub game_id: Option<String>,
    pub name: Option<String>,
    pub max_players: Option<i32>,
}

async fn create_room_handler(
    State(_state): State<AppState>,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Envelope<Value>>) {
    let payload = json!({
        "room": {
            "id": uuid::Uuid::new_v4().to_string(),
            "game_id": body.get("game_id").cloned().unwrap_or(Value::Null),
            "status": "waiting",
            "created_at": Utc::now().to_rfc3339(),
        },
        "protocol_version": PROTOCOL_VERSION,
    });
    (StatusCode::CREATED, Json(Envelope::new("rooms.create.ok", payload)))
}

async fn get_room_handler(
    State(_state): State<AppState>,
    Path(id): Path<String>,
) -> (StatusCode, Json<Envelope<Value>>) {
    let payload = json!({
        "room": { "id": id, "status": "waiting", "players": [] },
        "protocol_version": PROTOCOL_VERSION,
    });
    (StatusCode::OK, Json(Envelope::new("rooms.get.ok", payload)))
}

async fn accept_room_handler(
    State(_state): State<AppState>,
    Path(id): Path<String>,
) -> (StatusCode, Json<Envelope<Value>>) {
    let payload = json!({
        "room_id": id,
        "status": "accepted",
        "protocol_version": PROTOCOL_VERSION,
    });
    (StatusCode::OK, Json(Envelope::new("rooms.accept.ok", payload)))
}

async fn decline_room_handler(
    State(_state): State<AppState>,
    Path(id): Path<String>,
) -> (StatusCode, Json<Envelope<Value>>) {
    let payload = json!({
        "room_id": id,
        "status": "declined",
        "protocol_version": PROTOCOL_VERSION,
    });
    (StatusCode::OK, Json(Envelope::new("rooms.decline.ok", payload)))
}

async fn cancel_room_handler(
    State(_state): State<AppState>,
    Path(id): Path<String>,
) -> (StatusCode, Json<Envelope<Value>>) {
    let payload = json!({
        "room_id": id,
        "status": "cancelled",
        "protocol_version": PROTOCOL_VERSION,
    });
    (StatusCode::OK, Json(Envelope::new("rooms.cancel.ok", payload)))
}

// ---------------------------------------------------------------------------
// WebSocket
// ---------------------------------------------------------------------------

async fn ws_handler(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    // Register this connection in the hub so other handlers could push messages.
    // Generates a random connection id and an unbounded channel for outbound messages.
    let conn_id = uuid::Uuid::new_v4().to_string();
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
    state.ws_hub.insert(conn_id.clone(), tx);
    info!(conn_id = %conn_id, hub_size = state.ws_hub.len(), "ws: connected");

    let hello = Envelope::new(
        "connection.hello",
        json!({ "message": "connected", "protocol_version": PROTOCOL_VERSION, "connection_id": conn_id }),
    );
    if let Ok(text) = serde_json::to_string(&hello) {
        if socket.send(Message::Text(text)).await.is_err() {
            state.ws_hub.remove(&conn_id);
            return;
        }
    }

    info!("ws: hello sent, entering recv loop");

    loop {
        tokio::select! {
            // Outbound messages queued via ws_hub
            msg = rx.recv() => {
                match msg {
                    Some(m) => {
                        if socket.send(m).await.is_err() {
                            break;
                        }
                    }
                    None => break,
                }
            }
            // Inbound messages from client
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        let incoming_value: Result<Value, _> = serde_json::from_str(&text);
                        let payload = match incoming_value {
                            Ok(v) => v,
                            Err(_) => Value::String(text),
                        };
                        let echo = Envelope::new("echo", payload);
                        if let Ok(out) = serde_json::to_string(&echo) {
                            if socket.send(Message::Text(out)).await.is_err() {
                                break;
                            }
                        }
                    }
                    Some(Ok(Message::Binary(bin))) => {
                        let echo = Envelope::new(
                            "echo.binary",
                            json!({ "bytes": bin.len(), "protocol_version": PROTOCOL_VERSION }),
                        );
                        if let Ok(out) = serde_json::to_string(&echo) {
                            let _ = socket.send(Message::Text(out)).await;
                        }
                    }
                    Some(Ok(Message::Close(_))) => break,
                    Some(Ok(Message::Ping(data))) => {
                        let _ = socket.send(Message::Pong(data)).await;
                    }
                    Some(Ok(Message::Pong(_))) => {}
                    Some(Err(_)) | None => break,
                }
            }
        }
    }

    state.ws_hub.remove(&conn_id);
    info!(conn_id = %conn_id, "ws: connection closed");
}

// ---------------------------------------------------------------------------
// Graceful shutdown
// ---------------------------------------------------------------------------

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => sig.recv().await,
            Err(_) => std::future::pending::<Option<()>>().await,
        };
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("signal received, starting graceful shutdown");
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    let config = Config::from_env();

    // Init tracing
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(config.rust_log.clone()));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .init();

    info!(
        "starting openfight-server protocol_version={} port={} rust_log={}",
        PROTOCOL_VERSION, config.port, config.rust_log
    );

    let pool = PgPool::connect(&config.database_url)
        .await
        .expect("failed to connect to DATABASE_URL — is Postgres running? check DATABASE_URL env");

    info!("database pool connected");

    let state = AppState {
        pool,
        config: config.clone(),
        ws_hub: Arc::new(DashMap::new()),
    };

    // CORS — permissive for development
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::any())
        .allow_headers(AllowHeaders::any())
        .allow_methods(AllowMethods::any())
        .allow_private_network(true);

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler))
        .route("/api/v1/auth/register", post(register_handler))
        .route("/api/v1/auth/login", post(login_handler))
        .route("/api/v1/auth/logout", post(logout_handler))
        .route("/api/v1/games", get(list_games_handler))
        .route("/api/v1/games/:id", get(get_game_handler))
        .route("/api/v1/servers", get(list_servers_handler))
        .route("/api/v1/lobbies/:game_id", get(list_lobbies_handler))
        .route("/api/v1/rooms", post(create_room_handler))
        .route("/api/v1/rooms/:id", get(get_room_handler))
        .route("/api/v1/rooms/:id/accept", post(accept_room_handler))
        .route("/api/v1/rooms/:id/decline", post(decline_room_handler))
        .route("/api/v1/rooms/:id/cancel", post(cancel_room_handler))
        .route("/ws", get(ws_handler))
        .with_state(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    info!(
        "openfight-server listening on {} (protocol {})",
        addr, PROTOCOL_VERSION
    );
    println!(
        "openfight-server listening on http://{} (protocol {})",
        addr, PROTOCOL_VERSION
    );

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind address");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server error");
}
