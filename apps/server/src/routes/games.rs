use axum::{Json, extract::Path, extract::State};
use opencade_protocol::Envelope;
use serde_json::{Value, json};
use sqlx::Row;

use crate::{authn::AuthUser, error::AppError, state::AppState};

pub async fn list_games(
    State(state): State<AppState>,
    _user: AuthUser,
) -> Result<Json<Envelope<Value>>, AppError> {
    let rows = sqlx::query(
        "SELECT games.id, games.name, games.emulator,
                game_versions.version AS default_version
         FROM games
         LEFT JOIN game_versions
           ON game_versions.game_id = games.id AND game_versions.is_default
         ORDER BY games.name
         LIMIT 500",
    )
    .fetch_all(&state.pool)
    .await?;
    let games = rows
        .into_iter()
        .map(|row| -> Result<Value, AppError> {
            Ok(json!({
                "id": required_string(&row, "id")?,
                "name": required_string(&row, "name")?,
                "emulator": required_string(&row, "emulator")?,
                "default_version": row.try_get::<Option<String>, _>("default_version")
                    .map_err(|_| AppError::Internal("invalid game record".into()))?,
            }))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(Envelope::new("games.list", json!({ "games": games }))))
}

pub async fn get_game(
    State(state): State<AppState>,
    _user: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<Envelope<Value>>, AppError> {
    if id.trim().is_empty() {
        return Err(AppError::BadRequest("game id must not be empty".into()));
    }
    let row = sqlx::query(
        "SELECT games.id, games.name, games.emulator,
                game_versions.version AS default_version
         FROM games
         LEFT JOIN game_versions
           ON game_versions.game_id = games.id AND game_versions.is_default
         WHERE games.id = $1",
    )
    .bind(&id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("game not found: {id}")))?;
    Ok(Json(Envelope::new(
        "games.get",
        json!({
            "game": {
                "id": required_string(&row, "id")?,
                "name": required_string(&row, "name")?,
                "emulator": required_string(&row, "emulator")?,
                "default_version": row.try_get::<Option<String>, _>("default_version")
                    .map_err(|_| AppError::Internal("invalid game record".into()))?,
            }
        }),
    )))
}

fn required_string(row: &sqlx::postgres::PgRow, field: &str) -> Result<String, AppError> {
    row.try_get(field)
        .map_err(|_| AppError::Internal("invalid game record".into()))
}
