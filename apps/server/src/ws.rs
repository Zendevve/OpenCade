use axum::{
    extract::{
        State,
        ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade, close_code},
    },
    http::{HeaderMap, header},
    response::{IntoResponse, Response},
};
use futures_util::{SinkExt, StreamExt};
use opencade_protocol::{BorrowedEnvelope, Envelope, PROTOCOL_VERSION, is_supported_version};
use serde::Deserialize;
use serde_json::{Value, json, value::RawValue};
use sqlx::Row;
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    authn::{AuthUser, authenticate_token},
    error::AppError,
    state::AppState,
};

const MAX_TEXT_BYTES: usize = 16 * 1024;
const OUTBOUND_QUEUE_CAPACITY: usize = 64;
const RATE_LIMIT_MESSAGES: usize = 30;
const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(10);

#[derive(Debug, Default)]
struct RateLimiter {
    received_at: VecDeque<Instant>,
}

impl RateLimiter {
    fn allow(&mut self, now: Instant) -> bool {
        while self
            .received_at
            .front()
            .is_some_and(|received| now.duration_since(*received) >= RATE_LIMIT_WINDOW)
        {
            self.received_at.pop_front();
        }
        if self.received_at.len() >= RATE_LIMIT_MESSAGES {
            return false;
        }
        self.received_at.push_back(now);
        true
    }
}

pub async fn ws_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Result<Response, AppError> {
    let token = websocket_token(&headers)
        .ok_or_else(|| AppError::Unauthorized("websocket authentication required".into()))?;
    let user = authenticate_token(&state, token).await?;
    Ok(upgrade
        .protocols(["opencade.v1"])
        .on_upgrade(move |socket| handle_socket(socket, state, user))
        .into_response())
}

fn websocket_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::SEC_WEBSOCKET_PROTOCOL)?
        .to_str()
        .ok()?
        .split(',')
        .map(str::trim)
        .find_map(|protocol| protocol.strip_prefix("opencade.auth."))
        .filter(|token| !token.is_empty())
}

pub async fn handle_socket(socket: WebSocket, state: AppState, user: AuthUser) {
    let user_key = user.id;
    let (sender, mut receiver) = mpsc::channel::<Message>(OUTBOUND_QUEUE_CAPACITY);
    let connection_sender = sender.clone();
    if let Some(previous) = state.ws_hub.insert(user_key, sender) {
        let _ = previous.try_send(Message::Close(Some(CloseFrame {
            code: close_code::NORMAL,
            reason: "replaced by a newer connection".into(),
        })));
    }

    let (mut socket_sink, mut socket_stream) = socket.split();
    let mut writer = tokio::spawn(async move {
        while let Some(message) = receiver.recv().await {
            socket_sink.send(message).await.map_err(|_| ())?;
        }
        Ok::<(), ()>(())
    });

    if send_envelope(
        &connection_sender,
        Envelope::new(
            "connection.hello",
            json!({ "user_id": user.id, "protocol_version": PROTOCOL_VERSION }),
        ),
    )
    .await
    .is_err()
    {
        state.ws_hub.remove(&user_key);
        writer.abort();
        return;
    }

    info!(user_id = %user.id, "websocket connected");
    let mut rate_limiter = RateLimiter::default();
    loop {
        tokio::select! {
            _ = &mut writer => break,
            inbound = socket_stream.next() => {
                match inbound {
                    Some(Ok(Message::Text(text))) => {
                        if !rate_limiter.allow(Instant::now()) {
                            if send_error(&connection_sender, "rate_limited", "message rate limit exceeded", None).await.is_err() {
                                break;
                            }
                            continue;
                        }
                        let original = Message::Text(text.clone());
                        if handle_text(&connection_sender, &state, &user, &text, &original).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        if connection_sender.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Pong(_))) => {}
                    Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                    Some(Ok(Message::Binary(_))) => {
                        if send_error(&connection_sender, "binary_not_supported", "binary frames are not accepted", None).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }
    }
    state.ws_hub.remove_if(&user_key, |_, current| {
        current.same_channel(&connection_sender)
    });
    writer.abort();
    info!(user_id = %user.id, "websocket disconnected");
}

async fn handle_text(
    sender: &mpsc::Sender<Message>,
    state: &AppState,
    user: &AuthUser,
    text: &str,
    original: &Message,
) -> Result<(), ()> {
    if text.len() > MAX_TEXT_BYTES {
        return send_error(sender, "payload_too_large", "message exceeds 16 KiB", None).await;
    }
    let envelope = match serde_json::from_str::<BorrowedEnvelope<'_>>(text) {
        Ok(envelope) => envelope,
        Err(_) => {
            return send_error(sender, "bad_request", "invalid envelope", None).await;
        }
    };
    if !is_supported_version(envelope.version) {
        return send_error(
            sender,
            "version_unsupported",
            "unsupported protocol version",
            Some(envelope.request_id),
        )
        .await;
    }
    if envelope.validate().is_err() {
        return send_error(
            sender,
            "bad_request",
            "invalid envelope",
            Some(envelope.request_id),
        )
        .await;
    }

    match envelope.msg_type {
        "ping" | "connection.ping" => {
            send_envelope(
                sender,
                Envelope::reply("pong", envelope.request_id, json!({})),
            )
            .await
        }
        "signaling.offer"
        | "signaling.answer"
        | "signaling.candidate"
        | "match.endpoint"
        | "match.probe.completed" => {
            let room_id = match envelope.msg_type {
                "match.endpoint" => validate_match_endpoint(envelope.payload)
                    .map_err(|_| ("invalid_candidate", "match endpoint candidate is invalid")),
                "match.probe.completed" => validate_match_completion(envelope.payload)
                    .map_err(|_| ("invalid_probe_report", "match probe completion is invalid")),
                _ => parse_room_id(envelope.payload)
                    .map_err(|_| ("bad_request", "payload must contain a valid room_id")),
            };
            let room_id = match room_id {
                Ok(room_id) => room_id,
                Err((code, message)) => {
                    return send_error(sender, code, message, Some(envelope.request_id)).await;
                }
            };
            if let Err(error) = relay_to_room_members(state, user.id, room_id, original).await {
                return send_error(
                    sender,
                    error.code(),
                    error.message(),
                    Some(envelope.request_id),
                )
                .await;
            }
            send_envelope(
                sender,
                Envelope::reply(
                    match envelope.msg_type {
                        "match.endpoint" => "match.endpoint.relayed",
                        "match.probe.completed" => "match.probe.completed.relayed",
                        _ => "signaling.relayed",
                    },
                    envelope.request_id,
                    json!({ "status": "relayed" }),
                ),
            )
            .await
        }
        _ => {
            send_error(
                sender,
                "unknown_type",
                "unknown message type",
                Some(envelope.request_id),
            )
            .await
        }
    }
}

#[derive(Deserialize)]
struct RoomPayload<'a> {
    #[serde(borrow)]
    room_id: &'a str,
}

#[derive(Deserialize)]
struct MatchEndpointPayloadRef<'a> {
    #[serde(borrow)]
    room_id: &'a str,
    #[serde(borrow)]
    endpoint: &'a str,
    #[serde(default, borrow)]
    reflexive_endpoint: Option<&'a str>,
    #[serde(borrow)]
    nonce: &'a str,
}

#[derive(Deserialize)]
struct MatchProbeCompletedPayloadRef<'a> {
    #[serde(borrow)]
    room_id: &'a str,
    frames_received: u32,
    #[serde(borrow)]
    transcript_checksum: &'a str,
}

fn parse_room_id(payload: &RawValue) -> Result<Uuid, ()> {
    let payload: RoomPayload<'_> = serde_json::from_str(payload.get()).map_err(|_| ())?;
    Uuid::parse_str(payload.room_id).map_err(|_| ())
}

fn validate_match_endpoint(payload: &RawValue) -> Result<Uuid, ()> {
    let candidate: MatchEndpointPayloadRef<'_> =
        serde_json::from_str(payload.get()).map_err(|_| ())?;
    candidate
        .endpoint
        .parse::<std::net::SocketAddr>()
        .map_err(|_| ())?;
    if let Some(reflexive) = candidate.reflexive_endpoint {
        reflexive.parse::<std::net::SocketAddr>().map_err(|_| ())?;
    }
    Uuid::parse_str(candidate.nonce).map_err(|_| ())?;
    Uuid::parse_str(candidate.room_id).map_err(|_| ())
}

fn validate_match_completion(payload: &RawValue) -> Result<Uuid, ()> {
    let completion: MatchProbeCompletedPayloadRef<'_> =
        serde_json::from_str(payload.get()).map_err(|_| ())?;
    if completion.frames_received == 0 || completion.frames_received > 10_000 {
        return Err(());
    }
    if completion.transcript_checksum.len() != 16
        || !completion
            .transcript_checksum
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(());
    }
    Uuid::parse_str(completion.room_id).map_err(|_| ())
}

async fn relay_to_room_members(
    state: &AppState,
    sender_id: Uuid,
    room_id: Uuid,
    original: &Message,
) -> Result<(), RelayError> {
    let row = sqlx::query(
        r#"SELECT
                EXISTS(
                    SELECT 1 FROM room_members
                    WHERE room_id = $1 AND user_id = $2
                ) AS authorized,
                (
                    SELECT user_id FROM room_members
                    WHERE room_id = $1 AND user_id <> $2
                    LIMIT 1
                ) AS peer_id"#,
    )
    .bind(room_id)
    .bind(sender_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|error| {
        warn!(%error, %room_id, "failed to load room members for signaling");
        RelayError::Database
    })?;
    let authorized = row
        .try_get::<bool, _>("authorized")
        .map_err(|_| RelayError::Database)?;
    if !authorized {
        return Err(RelayError::Forbidden);
    }
    let peer_id = row
        .try_get::<Option<Uuid>, _>("peer_id")
        .map_err(|_| RelayError::Database)?
        .ok_or(RelayError::PeerUnavailable)?;
    let target = state
        .ws_hub
        .get(&peer_id)
        .ok_or(RelayError::PeerUnavailable)?;
    target.try_send(original.clone()).map_err(|error| {
        warn!(%error, user_id = %peer_id, %room_id, "signaling queue unavailable");
        RelayError::PeerUnavailable
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RelayError {
    Forbidden,
    PeerUnavailable,
    Database,
}

impl RelayError {
    fn code(self) -> &'static str {
        match self {
            Self::Forbidden => "forbidden",
            Self::PeerUnavailable => "peer_unavailable",
            Self::Database => "internal_error",
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::Forbidden => "sender is not a room member",
            Self::PeerUnavailable => "peer signaling connection is unavailable",
            Self::Database => "unable to authorize signaling message",
        }
    }
}

async fn send_error(
    sender: &mpsc::Sender<Message>,
    code: &str,
    message: &str,
    request_id: Option<&str>,
) -> Result<(), ()> {
    let envelope = match request_id {
        Some(request_id) => Envelope::reply(
            "error",
            request_id,
            json!({ "code": code, "message": message }),
        ),
        None => Envelope::new("error", json!({ "code": code, "message": message })),
    };
    send_envelope(sender, envelope).await
}

async fn send_envelope(
    sender: &mpsc::Sender<Message>,
    envelope: Envelope<Value>,
) -> Result<(), ()> {
    let text = serde_json::to_string(&envelope).map_err(|_| ())?;
    sender
        .send(Message::Text(text.into()))
        .await
        .map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn websocket_limits_are_bounded() {
        assert_eq!(MAX_TEXT_BYTES, 16 * 1024);
        assert_eq!(OUTBOUND_QUEUE_CAPACITY, 64);
    }

    #[test]
    fn websocket_rate_limit_recovers_after_the_window() {
        let start = Instant::now();
        let mut limiter = RateLimiter::default();
        for _ in 0..RATE_LIMIT_MESSAGES {
            assert!(limiter.allow(start));
        }
        assert!(!limiter.allow(start));
        assert!(limiter.allow(start + RATE_LIMIT_WINDOW));
    }

    #[test]
    fn websocket_token_comes_from_subprotocol_not_uri() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::SEC_WEBSOCKET_PROTOCOL,
            "opencade.v1, opencade.auth.secret-token"
                .parse()
                .expect("protocol header"),
        );
        assert_eq!(websocket_token(&headers), Some("secret-token"));
    }

    #[test]
    fn match_endpoint_requires_a_socket_address_and_uuid_nonce() {
        let valid = RawValue::from_string(
            json!({
                "room_id": Uuid::new_v4(),
                "endpoint": "192.168.1.20:42000",
                "nonce": Uuid::new_v4()
            })
            .to_string(),
        )
        .expect("raw payload");
        assert!(validate_match_endpoint(&valid).is_ok());

        let invalid_endpoint = RawValue::from_string(
            json!({
                "room_id": Uuid::new_v4(),
                "endpoint": "not-an-endpoint",
                "nonce": Uuid::new_v4()
            })
            .to_string(),
        )
        .expect("raw payload");
        assert!(validate_match_endpoint(&invalid_endpoint).is_err());

        let invalid_nonce = RawValue::from_string(
            json!({
                "room_id": Uuid::new_v4(),
                "endpoint": "192.168.1.20:42000",
                "nonce": "predictable"
            })
            .to_string(),
        )
        .expect("raw payload");
        assert!(validate_match_endpoint(&invalid_nonce).is_err());
    }

    #[test]
    fn match_completion_requires_bounded_frames_and_a_checksum() {
        let valid = RawValue::from_string(
            json!({
                "room_id": Uuid::new_v4(),
                "frames_received": 60,
                "transcript_checksum": "0376c2e852f4fd25"
            })
            .to_string(),
        )
        .expect("raw payload");
        assert!(validate_match_completion(&valid).is_ok());

        let invalid = RawValue::from_string(
            json!({
                "room_id": Uuid::new_v4(),
                "frames_received": 0,
                "transcript_checksum": "not-a-checksum"
            })
            .to_string(),
        )
        .expect("raw payload");
        assert!(validate_match_completion(&invalid).is_err());
    }
}
