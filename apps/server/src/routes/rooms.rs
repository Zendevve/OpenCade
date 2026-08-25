use std::sync::atomic::Ordering;

use axum::{Json, extract::Path, extract::State, http::StatusCode};
use opencade_protocol::{Envelope, RoomPayload, RoomState};
use opencade_shared::RelayTicket;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

use crate::{
    authn::{AuthUser, generate_session_token, hash_token},
    error::AppError,
    room_state::{RoomEvent, from_database, to_database, transition},
    state::AppState,
};

const RETROARCH_ALPHA_PORT: u16 = 55_435;

#[derive(Debug, Deserialize)]
pub struct CreateRoomRequest {
    game_id: String,
    #[serde(default = "default_max_players")]
    max_players: i32,
}

#[derive(Debug, Deserialize)]
pub struct FinishRoomRequest {
    exit_code: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct CreateLaunchGrantRequest {
    local_endpoint: String,
    peer_endpoint: String,
    input_delay_frames: u8,
}

#[derive(Debug, Deserialize)]
pub struct ConsumeLaunchGrantRequest {
    grant: String,
}

fn default_max_players() -> i32 {
    2
}

async fn room_payload(
    state: &AppState,
    room_id: Uuid,
    requesting_user: Uuid,
) -> Result<RoomPayload, AppError> {
    let row = sqlx::query(
        "SELECT rooms.game_id, rooms.host_user_id, rooms.state
         FROM rooms
         JOIN room_members ON room_members.room_id = rooms.id
         WHERE rooms.id = $1 AND room_members.user_id = $2",
    )
    .bind(room_id)
    .bind(requesting_user)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("room not found: {room_id}")))?;
    let host_id: Uuid = row
        .try_get("host_user_id")
        .map_err(|_| AppError::Internal("invalid room record".into()))?;
    let guest_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT user_id FROM room_members
         WHERE room_id = $1 AND user_id <> $2
         ORDER BY joined_at LIMIT 1",
    )
    .bind(room_id)
    .bind(host_id)
    .fetch_optional(&state.pool)
    .await?;
    let state_value: String = row
        .try_get("state")
        .map_err(|_| AppError::Internal("invalid room record".into()))?;
    Ok(RoomPayload {
        id: room_id.to_string(),
        game_id: row
            .try_get("game_id")
            .map_err(|_| AppError::Internal("invalid room record".into()))?,
        host_id: host_id.to_string(),
        guest_id: guest_id.map(|id| id.to_string()),
        state: from_database(&state_value).map_err(AppError::Internal)?,
    })
}

async fn locked_room(
    transaction: &mut Transaction<'_, Postgres>,
    room_id: Uuid,
) -> Result<(Uuid, RoomState, i32), AppError> {
    let row =
        sqlx::query("SELECT host_user_id, state, max_players FROM rooms WHERE id = $1 FOR UPDATE")
            .bind(room_id)
            .fetch_optional(&mut **transaction)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("room not found: {room_id}")))?;
    let state: String = row
        .try_get("state")
        .map_err(|_| AppError::Internal("invalid room record".into()))?;
    Ok((
        row.try_get("host_user_id")
            .map_err(|_| AppError::Internal("invalid room record".into()))?,
        from_database(&state).map_err(AppError::Internal)?,
        row.try_get("max_players")
            .map_err(|_| AppError::Internal("invalid room record".into()))?,
    ))
}

pub async fn create_room(
    State(state): State<AppState>,
    user: AuthUser,
    Json(request): Json<CreateRoomRequest>,
) -> Result<(StatusCode, Json<Envelope<Value>>), AppError> {
    if request.game_id.trim().is_empty() || request.max_players != 2 {
        return Err(AppError::BadRequest(
            "game_id is required and alpha rooms support exactly two players".into(),
        ));
    }
    let game_exists =
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM games WHERE id = $1)")
            .bind(&request.game_id)
            .fetch_one(&state.pool)
            .await?;
    if !game_exists {
        return Err(AppError::NotFound(format!(
            "game not found: {}",
            request.game_id
        )));
    }

    if let Some(existing_id) = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM rooms
         WHERE host_user_id = $1 AND game_id = $2 AND state = 'WAITING'
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(user.id)
    .bind(&request.game_id)
    .fetch_optional(&state.pool)
    .await?
    {
        let payload = room_payload(&state, existing_id, user.id).await?;
        return Ok((
            StatusCode::OK,
            Json(Envelope::new("rooms.existing", json!(payload))),
        ));
    }

    let room_id = Uuid::new_v4();
    let mut transaction = state.pool.begin().await?;
    sqlx::query(
        "INSERT INTO rooms (id, game_id, host_user_id, state, max_players)
         VALUES ($1, $2, $3, 'WAITING', $4)",
    )
    .bind(room_id)
    .bind(&request.game_id)
    .bind(user.id)
    .bind(request.max_players)
    .execute(&mut *transaction)
    .await?;
    sqlx::query("INSERT INTO room_members (room_id, user_id) VALUES ($1, $2)")
        .bind(room_id)
        .bind(user.id)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    state.metrics.rooms_created.fetch_add(1, Ordering::Relaxed);

    let payload = room_payload(&state, room_id, user.id).await?;
    Ok((
        StatusCode::CREATED,
        Json(Envelope::new("rooms.created", json!(payload))),
    ))
}

pub async fn get_room(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Envelope<Value>>, AppError> {
    let payload = room_payload(&state, id, user.id).await?;
    Ok(Json(Envelope::new("rooms.get", json!(payload))))
}

pub async fn accept_room(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Envelope<Value>>, AppError> {
    let mut transaction = state.pool.begin().await?;
    let (host_id, current, max_players) = locked_room(&mut transaction, id).await?;
    if host_id == user.id {
        return Err(AppError::Forbidden(
            "room host cannot accept their own room".into(),
        ));
    }
    let members =
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM room_members WHERE room_id = $1")
            .bind(id)
            .fetch_one(&mut *transaction)
            .await?;
    if members >= i64::from(max_players) {
        return Err(AppError::Conflict("room is full".into()));
    }
    let next = transition(current, RoomEvent::Accept)
        .map_err(|error| AppError::Conflict(error.to_string()))?;
    sqlx::query(
        "INSERT INTO room_members (room_id, user_id) VALUES ($1, $2)
         ON CONFLICT (room_id, user_id) DO NOTHING",
    )
    .bind(id)
    .bind(user.id)
    .execute(&mut *transaction)
    .await?;
    sqlx::query("UPDATE rooms SET state = $2 WHERE id = $1")
        .bind(id)
        .bind(to_database(&next))
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;

    let payload = room_payload(&state, id, user.id).await?;
    Ok(Json(Envelope::new("rooms.accepted", json!(payload))))
}

pub async fn decline_room(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Envelope<Value>>, AppError> {
    change_room_state(&state, &user, id, RoomEvent::Decline, false).await?;
    Ok(Json(Envelope::new(
        "rooms.declined",
        json!({ "room_id": id, "state": "cancelled" }),
    )))
}

pub async fn cancel_room(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Envelope<Value>>, AppError> {
    change_room_state(&state, &user, id, RoomEvent::Cancel, true).await?;
    Ok(Json(Envelope::new(
        "rooms.cancelled",
        json!({ "room_id": id, "state": "cancelled" }),
    )))
}

pub async fn start_room(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Envelope<Value>>, AppError> {
    confirm_native_launch(&state, &user, id).await
}

pub async fn finish_room(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(request): Json<FinishRoomRequest>,
) -> Result<Json<Envelope<Value>>, AppError> {
    confirm_native_exit(&state, &user, id, request.exit_code).await
}

pub async fn relay_ticket(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Envelope<Value>>, AppError> {
    let relay_url = state
        .config
        .relay_url
        .as_deref()
        .ok_or_else(|| AppError::Conflict("relay fallback is not configured".into()))?;
    let relay_secret = state
        .config
        .relay_secret
        .as_deref()
        .ok_or_else(|| AppError::Internal("relay signing is not configured".into()))?;
    let eligible = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(
            SELECT 1 FROM room_members
            JOIN rooms ON rooms.id = room_members.room_id
            WHERE room_members.room_id = $1
              AND room_members.user_id = $2
              AND rooms.state IN ('CONNECTING', 'PLAYING')
         )",
    )
    .bind(id)
    .bind(user.id)
    .fetch_one(&state.pool)
    .await?;
    if !eligible {
        return Err(AppError::Forbidden(
            "relay tickets require active room membership".into(),
        ));
    }
    let expires_at = chrono::Utc::now().timestamp() + 120;
    let ticket = RelayTicket::issue(
        relay_secret.as_bytes(),
        &id.to_string(),
        &user.id.to_string(),
        expires_at,
    )
    .map_err(|_| AppError::Internal("failed to issue relay ticket".into()))?;
    Ok(Json(Envelope::new(
        "relay.ticket",
        json!({ "relay_url": relay_url, "ticket": ticket }),
    )))
}

pub async fn create_launch_grant(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(request): Json<CreateLaunchGrantRequest>,
) -> Result<Json<Envelope<Value>>, AppError> {
    let local_endpoint = request
        .local_endpoint
        .parse::<std::net::SocketAddr>()
        .map_err(|_| AppError::BadRequest("local_endpoint must be a numeric IP:port".into()))?;
    let peer_endpoint = request
        .peer_endpoint
        .parse::<std::net::SocketAddr>()
        .map_err(|_| AppError::BadRequest("peer_endpoint must be a numeric IP:port".into()))?;
    if local_endpoint.port() != RETROARCH_ALPHA_PORT
        || peer_endpoint.port() != RETROARCH_ALPHA_PORT
        || local_endpoint.ip().is_unspecified()
        || local_endpoint.ip().is_multicast()
        || peer_endpoint.ip().is_unspecified()
        || peer_endpoint.ip().is_multicast()
        || request.input_delay_frames > 15
    {
        return Err(AppError::BadRequest(
            "native alpha endpoints must be unicast on port 55435 and input delay must be at most 15"
                .into(),
        ));
    }

    let mut transaction = state.pool.begin().await?;
    let (host_id, current, _) = locked_room(&mut transaction, id).await?;
    if current != RoomState::Connecting {
        return Err(AppError::Conflict(
            "launch grants are issued only while a room is connecting".into(),
        ));
    }
    let row = sqlx::query(
        "SELECT rooms.game_id, peer.user_id AS peer_user_id
         FROM rooms
         JOIN room_members local ON local.room_id = rooms.id AND local.user_id = $2
         JOIN room_members peer ON peer.room_id = rooms.id AND peer.user_id <> $2
         WHERE rooms.id = $1
         ORDER BY peer.joined_at
         LIMIT 1",
    )
    .bind(id)
    .bind(user.id)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or_else(|| AppError::Forbidden("launch grant requires two room members".into()))?;
    let game_id: String = row
        .try_get("game_id")
        .map_err(|_| AppError::Internal("invalid room record".into()))?;
    let peer_user_id: Uuid = row
        .try_get("peer_user_id")
        .map_err(|_| AppError::Internal("invalid room member record".into()))?;
    let role = if user.id == host_id { "host" } else { "guest" };
    let grant = generate_session_token();
    let expires_at = chrono::Utc::now() + chrono::Duration::seconds(90);
    sqlx::query(
        "INSERT INTO match_launch_grants (
            token_hash, room_id, game_id, local_user_id, peer_user_id, role,
            local_endpoint, peer_endpoint, input_delay_frames, expires_at
         ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(hash_token(&grant))
    .bind(id)
    .bind(&game_id)
    .bind(user.id)
    .bind(peer_user_id)
    .bind(role)
    .bind(local_endpoint.to_string())
    .bind(peer_endpoint.to_string())
    .bind(i16::from(request.input_delay_frames))
    .bind(expires_at)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;

    Ok(Json(Envelope::new(
        "match.launch_grant.created",
        json!({ "grant": grant, "expires_at": expires_at }),
    )))
}

pub async fn consume_launch_grant(
    State(state): State<AppState>,
    user: AuthUser,
    Json(request): Json<ConsumeLaunchGrantRequest>,
) -> Result<Json<Envelope<Value>>, AppError> {
    if request.grant.len() < 32 || request.grant.len() > 256 {
        return Err(AppError::BadRequest("launch grant is invalid".into()));
    }
    let row = sqlx::query(
        "UPDATE match_launch_grants SET consumed_at = now()
         WHERE token_hash = $1 AND local_user_id = $2 AND expires_at > now()
           AND consumed_at IS NULL
         RETURNING room_id, game_id, local_user_id, peer_user_id, role,
                   local_endpoint, peer_endpoint, input_delay_frames",
    )
    .bind(hash_token(&request.grant))
    .bind(user.id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::Unauthorized("launch grant is invalid or expired".into()))?;

    Ok(Json(Envelope::new(
        "match.launch_grant.consumed",
        json!({
            "room_id": row.try_get::<Uuid, _>("room_id").map_err(|_| AppError::Internal("invalid launch grant".into()))?,
            "game_id": row.try_get::<String, _>("game_id").map_err(|_| AppError::Internal("invalid launch grant".into()))?,
            "local_user_id": row.try_get::<Uuid, _>("local_user_id").map_err(|_| AppError::Internal("invalid launch grant".into()))?,
            "peer_user_id": row.try_get::<Uuid, _>("peer_user_id").map_err(|_| AppError::Internal("invalid launch grant".into()))?,
            "role": row.try_get::<String, _>("role").map_err(|_| AppError::Internal("invalid launch grant".into()))?,
            "local_endpoint": row.try_get::<String, _>("local_endpoint").map_err(|_| AppError::Internal("invalid launch grant".into()))?,
            "peer_endpoint": row.try_get::<String, _>("peer_endpoint").map_err(|_| AppError::Internal("invalid launch grant".into()))?,
            "input_delay_frames": row.try_get::<i16, _>("input_delay_frames").map_err(|_| AppError::Internal("invalid launch grant".into()))?,
        }),
    )))
}

async fn confirm_native_launch(
    state: &AppState,
    user: &AuthUser,
    room_id: Uuid,
) -> Result<Json<Envelope<Value>>, AppError> {
    let mut transaction = state.pool.begin().await?;
    let (_, current, _) = locked_room(&mut transaction, room_id).await?;
    if !matches!(current, RoomState::Connecting | RoomState::Playing) {
        return Err(AppError::Conflict(
            "native launch confirmation requires a connecting or playing room".into(),
        ));
    }
    let member = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM room_members WHERE room_id = $1 AND user_id = $2)",
    )
    .bind(room_id)
    .bind(user.id)
    .fetch_one(&mut *transaction)
    .await?;
    if !member {
        return Err(AppError::Forbidden("not a room member".into()));
    }
    let existing_launch = sqlx::query_scalar::<_, bool>(
        "SELECT ended_at IS NOT NULL FROM match_runtime_participants
         WHERE room_id = $1 AND user_id = $2",
    )
    .bind(room_id)
    .bind(user.id)
    .fetch_optional(&mut *transaction)
    .await?;
    if let Some(ended) = existing_launch {
        if ended {
            return Err(AppError::Conflict(
                "a finished native process cannot be relaunched in the same room".into(),
            ));
        }
        transaction.commit().await?;
        return notify_room_state(state, room_id, user.id).await;
    }
    let authorized = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(
            SELECT 1 FROM match_launch_grants
            WHERE room_id = $1 AND local_user_id = $2
              AND consumed_at IS NOT NULL AND expires_at > now()
         )",
    )
    .bind(room_id)
    .bind(user.id)
    .fetch_one(&mut *transaction)
    .await?;
    if !authorized {
        return Err(AppError::Conflict(
            "native launch confirmation requires a consumed launch grant".into(),
        ));
    }
    sqlx::query(
        "INSERT INTO match_runtime_participants (room_id, user_id)
         VALUES ($1, $2)
         ON CONFLICT (room_id, user_id) DO NOTHING",
    )
    .bind(room_id)
    .bind(user.id)
    .execute(&mut *transaction)
    .await?;
    sqlx::query("DELETE FROM match_launch_grants WHERE room_id = $1 AND local_user_id = $2")
        .bind(room_id)
        .bind(user.id)
        .execute(&mut *transaction)
        .await?;

    let member_count =
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM room_members WHERE room_id = $1")
            .bind(room_id)
            .fetch_one(&mut *transaction)
            .await?;
    let launched_count = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM match_runtime_participants WHERE room_id = $1",
    )
    .bind(room_id)
    .fetch_one(&mut *transaction)
    .await?;

    if current == RoomState::Connecting && member_count == 2 && launched_count == member_count {
        let next = transition(current, RoomEvent::Start)
            .map_err(|error| AppError::Conflict(error.to_string()))?;
        sqlx::query("UPDATE rooms SET state = $2 WHERE id = $1")
            .bind(room_id)
            .bind(to_database(&next))
            .execute(&mut *transaction)
            .await?;
        sqlx::query(
            "INSERT INTO matches (room_id, game_id, started_at)
             SELECT id, game_id, now() FROM rooms WHERE id = $1
             ON CONFLICT (room_id) DO NOTHING",
        )
        .bind(room_id)
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;

    notify_room_state(state, room_id, user.id).await
}

async fn confirm_native_exit(
    state: &AppState,
    user: &AuthUser,
    room_id: Uuid,
    exit_code: Option<i32>,
) -> Result<Json<Envelope<Value>>, AppError> {
    let mut transaction = state.pool.begin().await?;
    let (_, current, _) = locked_room(&mut transaction, room_id).await?;
    if !matches!(
        current,
        RoomState::Connecting | RoomState::Playing | RoomState::Finished | RoomState::Cancelled
    ) {
        return Err(AppError::Conflict(
            "native exit confirmation requires an active or terminal match room".into(),
        ));
    }
    let updated = sqlx::query(
        "UPDATE match_runtime_participants
         SET ended_at = COALESCE(ended_at, now()), exit_code = COALESCE(exit_code, $3)
         WHERE room_id = $1 AND user_id = $2",
    )
    .bind(room_id)
    .bind(user.id)
    .bind(exit_code)
    .execute(&mut *transaction)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(AppError::Conflict(
            "native exit cannot be confirmed before launch".into(),
        ));
    }

    match current {
        RoomState::Connecting => {
            let next = transition(current, RoomEvent::Cancel)
                .map_err(|error| AppError::Conflict(error.to_string()))?;
            sqlx::query("UPDATE rooms SET state = $2 WHERE id = $1")
                .bind(room_id)
                .bind(to_database(&next))
                .execute(&mut *transaction)
                .await?;
        }
        RoomState::Playing => {
            let member_count = sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM room_members WHERE room_id = $1",
            )
            .bind(room_id)
            .fetch_one(&mut *transaction)
            .await?;
            let ended_count = sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM match_runtime_participants
                 WHERE room_id = $1 AND ended_at IS NOT NULL",
            )
            .bind(room_id)
            .fetch_one(&mut *transaction)
            .await?;
            if member_count == 2 && ended_count == member_count {
                let next = transition(current, RoomEvent::Finish)
                    .map_err(|error| AppError::Conflict(error.to_string()))?;
                sqlx::query("UPDATE rooms SET state = $2 WHERE id = $1")
                    .bind(room_id)
                    .bind(to_database(&next))
                    .execute(&mut *transaction)
                    .await?;
                sqlx::query(
                    "UPDATE matches SET ended_at = COALESCE(ended_at, now()) WHERE room_id = $1",
                )
                .bind(room_id)
                .execute(&mut *transaction)
                .await?;
            }
        }
        RoomState::Finished | RoomState::Cancelled => {}
        _ => return Err(AppError::Internal("invalid native lifecycle state".into())),
    }
    transaction.commit().await?;

    notify_room_state(state, room_id, user.id).await
}

async fn notify_room_state(
    state: &AppState,
    room_id: Uuid,
    requesting_user: Uuid,
) -> Result<Json<Envelope<Value>>, AppError> {
    let payload = room_payload(state, room_id, requesting_user).await?;
    let payload_value = serde_json::to_value(&payload)
        .map_err(|_| AppError::Internal("failed to serialize room".into()))?;
    let members =
        sqlx::query_scalar::<_, Uuid>("SELECT user_id FROM room_members WHERE room_id = $1")
            .bind(room_id)
            .fetch_all(&state.pool)
            .await?;
    for member_id in members {
        state.notify_user(member_id, "room.state", payload_value.clone());
    }
    Ok(Json(Envelope::new("room.state", payload_value)))
}

async fn change_room_state(
    state: &AppState,
    user: &AuthUser,
    room_id: Uuid,
    event: RoomEvent,
    host_only: bool,
) -> Result<(), AppError> {
    let mut transaction = state.pool.begin().await?;
    let (host_id, current, _) = locked_room(&mut transaction, room_id).await?;
    if host_only && host_id != user.id {
        return Err(AppError::Forbidden(
            "only the room host can cancel this room".into(),
        ));
    }
    if !host_only {
        let member = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(
                SELECT 1 FROM room_members WHERE room_id = $1 AND user_id = $2
             )",
        )
        .bind(room_id)
        .bind(user.id)
        .fetch_one(&mut *transaction)
        .await?;
        if !member {
            return Err(AppError::Forbidden("not a room member".into()));
        }
    }
    let next = transition(current, event).map_err(|error| AppError::Conflict(error.to_string()))?;
    sqlx::query("UPDATE rooms SET state = $2 WHERE id = $1")
        .bind(room_id)
        .bind(to_database(&next))
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_rooms_to_two_players() {
        assert_eq!(default_max_players(), 2);
    }
}
