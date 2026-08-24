use axum::{Json, extract::State};
use opencade_protocol::Envelope;
use serde_json::{Value, json};
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
    let stun_hint = if state.config.stun_host.is_empty() {
        None
    } else {
        Some(format!(
            "{}:{}",
            state.config.stun_host, state.config.stun_port
        ))
    };
    let servers = rows
        .into_iter()
        .map(|row| -> Result<Value, AppError> {
            let mut obj = json!({
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
            });
            if let Some(ref stun) = stun_hint {
                obj["stun"] = json!(stun);
            }
            Ok(obj)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(Envelope::new(
        "servers.list",
        json!({ "servers": servers }),
    )))
}
