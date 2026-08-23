use axum::{extract::State, Json};
use openfight_protocol::Envelope;
use serde_json::{json, Value};
use sqlx::Row;

use crate::{authn::AuthUser, error::AppError, state::AppState};

pub async fn list_servers(
    State(state): State<AppState>,
    _user: AuthUser,
) -> Result<Json<Envelope<Value>>, AppError> {
    let rows = sqlx::query(
        "SELECT id, name, region, host, port FROM servers ORDER BY region, name LIMIT 100",
    )
    .fetch_all(&state.pool)
    .await?;
    let servers = rows
        .into_iter()
        .map(|row| -> Result<Value, AppError> {
            Ok(json!({
                "id": row.try_get::<uuid::Uuid, _>("id")
                    .map_err(|_| AppError::Internal("invalid server record".into()))?
                    .to_string(),
                "name": row.try_get::<String, _>("name")
                    .map_err(|_| AppError::Internal("invalid server record".into()))?,
                "region": row.try_get::<String, _>("region")
                    .map_err(|_| AppError::Internal("invalid server record".into()))?,
                "host": row.try_get::<String, _>("host")
                    .map_err(|_| AppError::Internal("invalid server record".into()))?,
                "port": row.try_get::<i32, _>("port")
                    .map_err(|_| AppError::Internal("invalid server record".into()))?,
            }))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(Envelope::new(
        "servers.list",
        json!({ "servers": servers }),
    )))
}
