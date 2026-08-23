use axum::{extract::State, http::StatusCode, Json};
use openfight_protocol::{Envelope, PROTOCOL_VERSION};
use serde_json::{json, Value};

use crate::state::AppState;

pub async fn health() -> (StatusCode, Json<Envelope<Value>>) {
    (
        StatusCode::OK,
        Json(Envelope::new(
            "health.ok",
            json!({ "status": "ok", "version": PROTOCOL_VERSION }),
        )),
    )
}

pub async fn ready(State(state): State<AppState>) -> (StatusCode, Json<Envelope<Value>>) {
    if sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .is_ok()
    {
        (
            StatusCode::OK,
            Json(Envelope::new(
                "ready.ok",
                json!({ "status": "ready", "database": "connected" }),
            )),
        )
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(Envelope::new(
                "ready.error",
                json!({ "status": "not_ready", "database": "unavailable" }),
            )),
        )
    }
}
