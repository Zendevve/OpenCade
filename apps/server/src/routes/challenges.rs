use axum::{Json, extract::Path, extract::State, http::StatusCode};
use opencade_protocol::Envelope;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

use crate::{
    authn::AuthUser,
    error::AppError,
    room_state::{RoomEvent, to_database, transition},
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct CreateChallengeRequest {
    game_id: String,
    challenged_id: Uuid,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct ChallengeView {
    id: Uuid,
    room_id: Uuid,
    game_id: String,
    challenger_id: Uuid,
    challenged_id: Uuid,
    state: String,
}

async fn locked_challenge(
    transaction: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<ChallengeView, AppError> {
    let row = sqlx::query(
        "SELECT challenges.id, challenges.room_id, rooms.game_id,
                challenges.challenger_id, challenges.challenged_id, challenges.state
         FROM challenges
         JOIN rooms ON rooms.id = challenges.room_id
         WHERE challenges.id = $1
         FOR UPDATE OF challenges, rooms",
    )
    .bind(id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("challenge not found: {id}")))?;

    Ok(ChallengeView {
        id: row.try_get("id").map_err(invalid_record)?,
        room_id: row.try_get("room_id").map_err(invalid_record)?,
        game_id: row.try_get("game_id").map_err(invalid_record)?,
        challenger_id: row.try_get("challenger_id").map_err(invalid_record)?,
        challenged_id: row.try_get("challenged_id").map_err(invalid_record)?,
        state: row.try_get("state").map_err(invalid_record)?,
    })
}

fn invalid_record(_: sqlx::Error) -> AppError {
    AppError::Internal("invalid challenge record".into())
}

pub async fn create_challenge(
    State(state): State<AppState>,
    user: AuthUser,
    Json(request): Json<CreateChallengeRequest>,
) -> Result<(StatusCode, Json<Envelope<Value>>), AppError> {
    if request.game_id.trim().is_empty() {
        return Err(AppError::BadRequest("game_id is required".into()));
    }
    if request.challenged_id == user.id {
        return Err(AppError::BadRequest(
            "users cannot challenge themselves".into(),
        ));
    }

    let (target_exists, game_exists) = sqlx::query_as::<_, (bool, bool)>(
        "SELECT EXISTS(SELECT 1 FROM users WHERE id = $1),
                EXISTS(SELECT 1 FROM games WHERE id = $2)",
    )
    .bind(request.challenged_id)
    .bind(&request.game_id)
    .fetch_one(&state.pool)
    .await?;
    if !target_exists {
        return Err(AppError::NotFound("challenged user not found".into()));
    }
    if !game_exists {
        return Err(AppError::NotFound("game not found".into()));
    }

    let mut transaction = state.pool.begin().await?;
    sqlx::query(
        "UPDATE rooms SET state = 'CANCELLED'
         WHERE host_user_id = $1 AND game_id = $2 AND state = 'WAITING'",
    )
    .bind(user.id)
    .bind(&request.game_id)
    .execute(&mut *transaction)
    .await?;
    let room_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO rooms (id, game_id, host_user_id, state, max_players)
         VALUES ($1, $2, $3, 'CHALLENGING', 2)",
    )
    .bind(room_id)
    .bind(&request.game_id)
    .bind(user.id)
    .execute(&mut *transaction)
    .await?;
    sqlx::query("INSERT INTO room_members (room_id, user_id) VALUES ($1, $2)")
        .bind(room_id)
        .bind(user.id)
        .execute(&mut *transaction)
        .await?;
    let row = sqlx::query(
        "INSERT INTO challenges (room_id, challenger_id, challenged_id, state)
         VALUES ($1, $2, $3, 'PENDING')
         RETURNING id",
    )
    .bind(room_id)
    .bind(user.id)
    .bind(request.challenged_id)
    .fetch_one(&mut *transaction)
    .await?;
    let challenge_id: Uuid = row.try_get("id").map_err(invalid_record)?;
    transaction.commit().await?;

    let payload = json!({
        "id": challenge_id,
        "room_id": room_id,
        "game_id": request.game_id,
        "challenger_id": user.id,
        "challenged_id": request.challenged_id,
        "state": "pending"
    });
    state.notify_user(request.challenged_id, "challenge.created", payload.clone());
    Ok((
        StatusCode::CREATED,
        Json(Envelope::new("challenge.created", payload)),
    ))
}

pub async fn list_incoming(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<Json<Envelope<Value>>, AppError> {
    let rows = sqlx::query(
        "SELECT challenges.id, challenges.room_id, rooms.game_id,
                challenges.challenger_id, challenges.challenged_id, challenges.state
         FROM challenges
         JOIN rooms ON rooms.id = challenges.room_id
         WHERE challenges.challenged_id = $1 AND challenges.state = 'PENDING'
         ORDER BY challenges.created_at DESC
         LIMIT 50",
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;
    let challenges = rows
        .into_iter()
        .map(|row| {
            Ok(ChallengeView {
                id: row.try_get("id").map_err(invalid_record)?,
                room_id: row.try_get("room_id").map_err(invalid_record)?,
                game_id: row.try_get("game_id").map_err(invalid_record)?,
                challenger_id: row.try_get("challenger_id").map_err(invalid_record)?,
                challenged_id: row.try_get("challenged_id").map_err(invalid_record)?,
                state: row
                    .try_get::<String, _>("state")
                    .map_err(invalid_record)?
                    .to_ascii_lowercase(),
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    Ok(Json(Envelope::new(
        "challenges.incoming",
        json!({ "challenges": challenges }),
    )))
}

pub async fn accept_challenge(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Envelope<Value>>, AppError> {
    let mut transaction = state.pool.begin().await?;
    let mut challenge = locked_challenge(&mut transaction, id).await?;
    if challenge.challenged_id != user.id {
        return Err(AppError::Forbidden(
            "only the challenged user can accept".into(),
        ));
    }
    require_pending(&challenge)?;
    sqlx::query(
        "UPDATE rooms SET state = 'CANCELLED'
         WHERE host_user_id = $1 AND game_id = $2 AND state = 'WAITING'",
    )
    .bind(user.id)
    .bind(&challenge.game_id)
    .execute(&mut *transaction)
    .await?;
    sqlx::query("INSERT INTO room_members (room_id, user_id) VALUES ($1, $2)")
        .bind(challenge.room_id)
        .bind(user.id)
        .execute(&mut *transaction)
        .await?;
    let next = transition(opencade_protocol::RoomState::Challenging, RoomEvent::Accept)
        .map_err(|error| AppError::Conflict(error.to_string()))?;
    sqlx::query("UPDATE rooms SET state = $2 WHERE id = $1")
        .bind(challenge.room_id)
        .bind(to_database(&next))
        .execute(&mut *transaction)
        .await?;
    sqlx::query("UPDATE challenges SET state = 'ACCEPTED' WHERE id = $1")
        .bind(id)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    challenge.state = "accepted".into();
    let payload = serde_json::to_value(&challenge)
        .map_err(|_| AppError::Internal("failed to serialize challenge".into()))?;
    state.notify_user(
        challenge.challenger_id,
        "challenge.accepted",
        payload.clone(),
    );
    Ok(Json(Envelope::new("challenge.accepted", payload)))
}

pub async fn decline_challenge(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Envelope<Value>>, AppError> {
    resolve_challenge(&state, &user, id, false).await
}

pub async fn cancel_challenge(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Envelope<Value>>, AppError> {
    resolve_challenge(&state, &user, id, true).await
}

async fn resolve_challenge(
    state: &AppState,
    user: &AuthUser,
    id: Uuid,
    cancel: bool,
) -> Result<Json<Envelope<Value>>, AppError> {
    let mut transaction = state.pool.begin().await?;
    let mut challenge = locked_challenge(&mut transaction, id).await?;
    let authorized = if cancel {
        challenge.challenger_id == user.id
    } else {
        challenge.challenged_id == user.id
    };
    if !authorized {
        return Err(AppError::Forbidden("challenge ownership mismatch".into()));
    }
    require_pending(&challenge)?;
    let challenge_state = if cancel { "CANCELLED" } else { "DECLINED" };
    sqlx::query("UPDATE challenges SET state = $2 WHERE id = $1")
        .bind(id)
        .bind(challenge_state)
        .execute(&mut *transaction)
        .await?;
    sqlx::query("UPDATE rooms SET state = 'CANCELLED' WHERE id = $1")
        .bind(challenge.room_id)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;

    challenge.state = challenge_state.to_ascii_lowercase();
    let payload = serde_json::to_value(&challenge)
        .map_err(|_| AppError::Internal("failed to serialize challenge".into()))?;
    let message_type = if cancel {
        "challenge.cancelled"
    } else {
        "challenge.declined"
    };
    let peer = if cancel {
        challenge.challenged_id
    } else {
        challenge.challenger_id
    };
    state.notify_user(peer, message_type, payload.clone());
    Ok(Json(Envelope::new(message_type, payload)))
}

fn require_pending(challenge: &ChallengeView) -> Result<(), AppError> {
    if challenge.state == "PENDING" {
        Ok(())
    } else {
        Err(AppError::Conflict("challenge is no longer pending".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_guard_rejects_terminal_challenges() {
        let challenge = ChallengeView {
            id: Uuid::nil(),
            room_id: Uuid::nil(),
            game_id: "kof98".into(),
            challenger_id: Uuid::nil(),
            challenged_id: Uuid::new_v4(),
            state: "DECLINED".into(),
        };
        assert!(matches!(
            require_pending(&challenge),
            Err(AppError::Conflict(_))
        ));
    }
}
