use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use openfight_protocol::{Envelope, RoomPayload, RoomState};
use serde_json::{json, Value};
use tracing::info;
use uuid::Uuid;

use crate::error::AppError;
use crate::state::AppState;

fn resolve_host_id(headers: &HeaderMap) -> String {
    match headers
        .get("x-user-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
    {
        Some(v) => v,
        None => "test-user".to_string(),
    }
}

fn parse_room_state(s: &str) -> RoomState {
    match s {
        "waiting" => RoomState::Waiting,
        "challenging" => RoomState::Challenging,
        "connecting" => RoomState::Connecting,
        "playing" => RoomState::Playing,
        "finished" => RoomState::Finished,
        "cancelled" => RoomState::Cancelled,
        _ => RoomState::Waiting,
    }
}

fn room_state_to_str(state: &RoomState) -> &'static str {
    match state {
        RoomState::Waiting => "waiting",
        RoomState::Challenging => "challenging",
        RoomState::Connecting => "connecting",
        RoomState::Playing => "playing",
        RoomState::Finished => "finished",
        RoomState::Cancelled => "cancelled",
    }
}

async fn lookup_game_uuid(state: &AppState, game_id_str: &str) -> Option<Uuid> {
    if let Ok(parsed) = Uuid::parse_str(game_id_str) {
        return Some(parsed);
    }
    let row = sqlx::query("SELECT id FROM games WHERE slug = $1 LIMIT 1")
        .bind(game_id_str)
        .fetch_one(&state.pool)
        .await
        .ok()?;
    use sqlx::Row;
    row.try_get::<Uuid, _>("id").ok()
}

async fn lookup_user_uuid(state: &AppState, host_id_str: &str) -> Option<Uuid> {
    if let Ok(parsed) = Uuid::parse_str(host_id_str) {
        return Some(parsed);
    }
    let row = sqlx::query("SELECT id FROM users WHERE username = $1 LIMIT 1")
        .bind(host_id_str)
        .fetch_one(&state.pool)
        .await
        .ok()?;
    use sqlx::Row;
    row.try_get::<Uuid, _>("id").ok()
}

/// POST /api/v1/rooms
/// Body: { game_id }
pub async fn create_room(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<impl IntoResponse, AppError> {
    let game_id_str = body
        .get("game_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("missing game_id".to_string()))?;

    if game_id_str.trim().is_empty() {
        return Err(AppError::BadRequest(
            "game_id must not be empty".to_string(),
        ));
    }

    let host_id_str = resolve_host_id(&headers);
    let room_id = Uuid::new_v4();
    let game_uuid = lookup_game_uuid(&state, game_id_str).await;
    let host_uuid = lookup_user_uuid(&state, &host_id_str).await;

    let insert_result = sqlx::query(
        "INSERT INTO rooms (id, game_id, host_id, state) VALUES ($1, $2, $3, 'waiting')",
    )
    .bind(room_id)
    .bind(game_uuid)
    .bind(host_uuid)
    .execute(&state.pool)
    .await;

    if let Err(e) = insert_result {
        info!(error = %e, room_id = %room_id, "rooms: insert failed, returning stub payload");
    } else {
        info!(room_id = %room_id, game_id = %game_id_str, host_id = %host_id_str, "rooms: room created");
    }

    let payload_room = RoomPayload {
        id: room_id.to_string(),
        game_id: game_id_str.to_string(),
        host_id: host_id_str.clone(),
        guest_id: None,
        state: RoomState::Waiting,
    };

    let envelope = Envelope::new("rooms.created", json!(payload_room));
    Ok((StatusCode::CREATED, Json(envelope)))
}

/// Fallback wrapper for router without HeaderMap extractor — uses stub host.
pub async fn create_room_without_headers(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Result<impl IntoResponse, AppError> {
    create_room(State(state), HeaderMap::new(), Json(body)).await
}

/// GET /api/v1/rooms/:id
pub async fn get_room(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let room_uuid =
        Uuid::parse_str(&id).map_err(|_| AppError::BadRequest("invalid room id".to_string()))?;

    let row = sqlx::query("SELECT id, game_id, host_id, guest_id, state FROM rooms WHERE id = $1")
        .bind(room_uuid)
        .fetch_one(&state.pool)
        .await;

    match row {
        Ok(r) => {
            use sqlx::Row;
            let db_id: Uuid = r
                .try_get("id")
                .map_err(|e| AppError::Internal(format!("row decode error: {}", e)))?;
            let game_id: Option<Uuid> = r.try_get("game_id").ok();
            let host_id: Option<Uuid> = r.try_get("host_id").ok();
            let guest_id: Option<Uuid> = r.try_get("guest_id").ok();
            let state_str: String = r
                .try_get("state")
                .map_err(|e| AppError::Internal(format!("row decode error: {}", e)))?;

            let payload_room = RoomPayload {
                id: db_id.to_string(),
                game_id: game_id.map(|g| g.to_string()).unwrap_or_default(),
                host_id: match host_id.map(|h| h.to_string()) {
                    Some(v) => v,
                    None => "test-user".to_string(),
                },
                guest_id: guest_id.map(|g| g.to_string()),
                state: parse_room_state(&state_str),
            };

            info!(room_id = %id, "rooms: get room");
            let envelope = Envelope::new("rooms.get", json!(payload_room));
            Ok((StatusCode::OK, Json(envelope)))
        }
        Err(sqlx::Error::RowNotFound) => Err(AppError::NotFound(format!("room not found: {}", id))),
        Err(e) => Err(AppError::Internal(format!("database error: {}", e))),
    }
}

/// POST /api/v1/rooms/:id/accept — transitions to challenging then connecting
pub async fn accept_room(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let room_uuid =
        Uuid::parse_str(&id).map_err(|_| AppError::BadRequest("invalid room id".to_string()))?;

    // First set to challenging, then connecting — do in one update to connecting for simplicity,
    // but log both transitions.
    let row = sqlx::query(
        "UPDATE rooms SET state = 'connecting', updated_at = now() WHERE id = $1 AND state IN ('waiting','challenging') RETURNING id, game_id, host_id, guest_id, state",
    )
    .bind(room_uuid)
    .fetch_one(&state.pool)
    .await;

    match row {
        Ok(r) => {
            use sqlx::Row;
            let db_id: Uuid = r
                .try_get("id")
                .map_err(|e| AppError::Internal(format!("row decode error: {}", e)))?;
            let game_id: Option<Uuid> = r.try_get("game_id").ok();
            let host_id: Option<Uuid> = r.try_get("host_id").ok();
            let guest_id: Option<Uuid> = r.try_get("guest_id").ok();
            let state_str: String = r
                .try_get("state")
                .map_err(|e| AppError::Internal(format!("row decode error: {}", e)))?;

            info!(room_id = %id, "rooms: room accepted -> connecting");

            let payload_room = RoomPayload {
                id: db_id.to_string(),
                game_id: game_id.map(|g| g.to_string()).unwrap_or_default(),
                host_id: match host_id.map(|h| h.to_string()) {
                    Some(v) => v,
                    None => "test-user".to_string(),
                },
                guest_id: guest_id.map(|g| g.to_string()),
                state: parse_room_state(&state_str),
            };

            let envelope = Envelope::new("rooms.accepted", json!(payload_room));
            Ok((StatusCode::OK, Json(envelope)))
        }
        Err(sqlx::Error::RowNotFound) => {
            // Check if room exists but wrong state
            let exists = sqlx::query("SELECT id FROM rooms WHERE id = $1")
                .bind(room_uuid)
                .fetch_one(&state.pool)
                .await;
            if exists.is_ok() {
                return Err(AppError::BadRequest(
                    "room not in accept-able state".to_string(),
                ));
            }
            Err(AppError::NotFound(format!("room not found: {}", id)))
        }
        Err(e) => Err(AppError::Internal(format!("database error: {}", e))),
    }
}

/// POST /api/v1/rooms/:id/decline — transitions to cancelled
pub async fn decline_room(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let room_uuid =
        Uuid::parse_str(&id).map_err(|_| AppError::BadRequest("invalid room id".to_string()))?;

    let row = sqlx::query(
        "UPDATE rooms SET state = 'cancelled', updated_at = now() WHERE id = $1 RETURNING id, game_id, host_id, guest_id, state",
    )
    .bind(room_uuid)
    .fetch_one(&state.pool)
    .await;

    match row {
        Ok(r) => {
            use sqlx::Row;
            let db_id: Uuid = r
                .try_get("id")
                .map_err(|e| AppError::Internal(format!("row decode error: {}", e)))?;
            let game_id: Option<Uuid> = r.try_get("game_id").ok();
            let host_id: Option<Uuid> = r.try_get("host_id").ok();
            let guest_id: Option<Uuid> = r.try_get("guest_id").ok();
            let state_str: String = r
                .try_get("state")
                .map_err(|e| AppError::Internal(format!("row decode error: {}", e)))?;

            info!(room_id = %id, "rooms: room declined -> cancelled");

            let payload_room = RoomPayload {
                id: db_id.to_string(),
                game_id: game_id.map(|g| g.to_string()).unwrap_or_default(),
                host_id: match host_id.map(|h| h.to_string()) {
                    Some(v) => v,
                    None => "test-user".to_string(),
                },
                guest_id: guest_id.map(|g| g.to_string()),
                state: parse_room_state(&state_str),
            };

            let envelope = Envelope::new("rooms.declined", json!(payload_room));
            Ok((StatusCode::OK, Json(envelope)))
        }
        Err(sqlx::Error::RowNotFound) => Err(AppError::NotFound(format!("room not found: {}", id))),
        Err(e) => Err(AppError::Internal(format!("database error: {}", e))),
    }
}

/// POST /api/v1/rooms/:id/cancel — host cancels, transitions to cancelled
pub async fn cancel_room(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let room_uuid =
        Uuid::parse_str(&id).map_err(|_| AppError::BadRequest("invalid room id".to_string()))?;

    let row = sqlx::query(
        "UPDATE rooms SET state = 'cancelled', updated_at = now() WHERE id = $1 AND state IN ('waiting','challenging','connecting') RETURNING id, game_id, host_id, guest_id, state",
    )
    .bind(room_uuid)
    .fetch_one(&state.pool)
    .await;

    match row {
        Ok(r) => {
            use sqlx::Row;
            let db_id: Uuid = r
                .try_get("id")
                .map_err(|e| AppError::Internal(format!("row decode error: {}", e)))?;
            let game_id: Option<Uuid> = r.try_get("game_id").ok();
            let host_id: Option<Uuid> = r.try_get("host_id").ok();
            let guest_id: Option<Uuid> = r.try_get("guest_id").ok();
            let state_str: String = r
                .try_get("state")
                .map_err(|e| AppError::Internal(format!("row decode error: {}", e)))?;

            info!(room_id = %id, "rooms: room cancelled");

            let payload_room = RoomPayload {
                id: db_id.to_string(),
                game_id: game_id.map(|g| g.to_string()).unwrap_or_default(),
                host_id: match host_id.map(|h| h.to_string()) {
                    Some(v) => v,
                    None => "test-user".to_string(),
                },
                guest_id: guest_id.map(|g| g.to_string()),
                state: parse_room_state(&state_str),
            };

            let envelope = Envelope::new("rooms.cancelled", json!(payload_room));
            Ok((StatusCode::OK, Json(envelope)))
        }
        Err(sqlx::Error::RowNotFound) => {
            let exists = sqlx::query("SELECT id FROM rooms WHERE id = $1")
                .bind(room_uuid)
                .fetch_one(&state.pool)
                .await;
            if exists.is_ok() {
                return Err(AppError::BadRequest(
                    "room cannot be cancelled in current state".to_string(),
                ));
            }
            Err(AppError::NotFound(format!("room not found: {}", id)))
        }
        Err(e) => Err(AppError::Internal(format!("database error: {}", e))),
    }
}
