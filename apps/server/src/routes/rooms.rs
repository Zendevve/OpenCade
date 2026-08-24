use std::sync::atomic::Ordering;

use axum::{Json, extract::Path, extract::State, http::StatusCode};
use opencade_protocol::{Envelope, RoomPayload, RoomState};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

use crate::{
    authn::AuthUser,
    error::AppError,
    room_state::{RoomEvent, from_database, to_database, transition},
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct CreateRoomRequest {
    game_id: String,
    #[serde(default = "default_max_players")]
    max_players: i32,
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
    if request.game_id.trim().is_empty() || !(2..=4).contains(&request.max_players) {
        return Err(AppError::BadRequest(
            "game_id is required and max_players must be between 2 and 4".into(),
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
    advance_match(&state, &user, id, RoomEvent::Start).await
}

pub async fn finish_room(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Envelope<Value>>, AppError> {
    advance_match(&state, &user, id, RoomEvent::Finish).await
}

async fn advance_match(
    state: &AppState,
    user: &AuthUser,
    room_id: Uuid,
    event: RoomEvent,
) -> Result<Json<Envelope<Value>>, AppError> {
    let mut transaction = state.pool.begin().await?;
    let (_, current, _) = locked_room(&mut transaction, room_id).await?;
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
    let next = transition(current, event).map_err(|error| AppError::Conflict(error.to_string()))?;
    sqlx::query("UPDATE rooms SET state = $2 WHERE id = $1")
        .bind(room_id)
        .bind(to_database(&next))
        .execute(&mut *transaction)
        .await?;
    match event {
        RoomEvent::Start => {
            sqlx::query(
                "INSERT INTO matches (room_id, game_id, started_at)
                 SELECT id, game_id, now() FROM rooms WHERE id = $1",
            )
            .bind(room_id)
            .execute(&mut *transaction)
            .await?;
        }
        RoomEvent::Finish => {
            sqlx::query(
                "UPDATE matches SET ended_at = now()
                 WHERE room_id = $1 AND ended_at IS NULL",
            )
            .bind(room_id)
            .execute(&mut *transaction)
            .await?;
        }
        _ => return Err(AppError::Internal("unsupported match event".into())),
    }
    transaction.commit().await?;

    let payload = room_payload(state, room_id, user.id).await?;
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
