use axum::{Json, extract::Path, extract::State};
use opencade_protocol::Envelope;
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::Row;

use crate::{authn::AuthUser, error::AppError, state::AppState};

#[derive(Debug, Serialize)]
pub struct LobbyMember {
    pub user_id: String,
    pub username: String,
    pub rtt_ms: Option<u32>,
    pub loss: Option<f32>,
    pub jitter_ms: Option<u32>,
    pub relay_reachable: Option<bool>,
}

pub async fn get_lobby(
    State(state): State<AppState>,
    _user: AuthUser,
    Path(game_id): Path<String>,
) -> Result<Json<Envelope<Value>>, AppError> {
    let exists = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM games WHERE id = $1)")
        .bind(&game_id)
        .fetch_one(&state.pool)
        .await?;
    if !exists {
        return Err(AppError::NotFound(format!("game not found: {game_id}")));
    }

    let rows = sqlx::query(
        "SELECT DISTINCT users.id, users.username
         FROM room_members
         JOIN rooms ON rooms.id = room_members.room_id
         JOIN users ON users.id = room_members.user_id
         WHERE rooms.game_id = $1
           AND rooms.state IN ('WAITING', 'CHALLENGING', 'CONNECTING', 'PLAYING')
         ORDER BY users.username
         LIMIT 500",
    )
    .bind(&game_id)
    .fetch_all(&state.pool)
    .await?;
    let members = rows
        .into_iter()
        .map(|row| -> Result<LobbyMember, AppError> {
            Ok(LobbyMember {
                user_id: row
                    .try_get::<uuid::Uuid, _>("id")
                    .map_err(|_| AppError::Internal("invalid lobby member record".into()))?
                    .to_string(),
                username: row
                    .try_get("username")
                    .map_err(|_| AppError::Internal("invalid lobby member record".into()))?,
                rtt_ms: None,
                loss: None,
                jitter_ms: None,
                relay_reachable: None,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Json(Envelope::new(
        "lobbies.get",
        json!({ "game_id": game_id, "members": members }),
    )))
}
