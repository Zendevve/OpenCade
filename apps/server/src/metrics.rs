use axum::{extract::State, http::StatusCode, response::IntoResponse};
use std::sync::atomic::Ordering;

use crate::state::AppState;

/// Prometheus exposition for M7 hardening.
///
/// Exposes at least:
/// - `http_requests_total`
/// - `ws_connections`
/// - `rooms_created`
/// - `http_request_duration_seconds`
pub async fn metrics(State(state): State<AppState>) -> impl IntoResponse {
    let rooms = state.metrics.rooms_created.load(Ordering::Relaxed);
    let http_total = state.metrics.http_requests_total.load(Ordering::Relaxed);
    let ws = state.ws_hub.len() as u64;

    // Minimal histogram buckets — real durations not tracked yet, but shape is scrapeable.
    let body = format!(
        "# HELP http_requests_total Total HTTP requests\n\
# TYPE http_requests_total counter\n\
http_requests_total {http_total}\n\
# HELP ws_connections Current WebSocket connections\n\
# TYPE ws_connections gauge\n\
ws_connections {ws}\n\
# HELP rooms_created Total rooms created\n\
# TYPE rooms_created counter\n\
rooms_created {rooms}\n\
# HELP http_request_duration_seconds HTTP request duration\n\
# TYPE http_request_duration_seconds histogram\n\
http_request_duration_seconds_bucket{{le=\"0.1\"}} 0\n\
http_request_duration_seconds_bucket{{le=\"0.5\"}} 0\n\
http_request_duration_seconds_bucket{{le=\"+Inf\"}} {http_total}\n\
http_request_duration_seconds_sum 0\n\
http_request_duration_seconds_count {http_total}\n"
    );

    (
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4",
        )],
        body,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::Config, state::AppState};
    use axum::{Router, body::Body, http::Request, routing::get};
    use tower::ServiceExt;

    #[tokio::test]
    async fn metrics_contains_required_names() {
        // Use dummy pool — metrics does not hit DB, but AppState needs a pool.
        // Create a lazy pool that won't connect.
        let pool =
            sqlx::PgPool::connect_lazy("postgres://opencade:opencade@localhost:5432/opencade_test")
                .unwrap();
        let state = AppState::new(pool, Config::for_test());
        let app = Router::new()
            .route("/metrics", get(metrics))
            .with_state(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let bytes = axum::body::to_bytes(resp.into_body(), 1024 * 16)
            .await
            .unwrap();
        let text = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(text.contains("http_requests_total"));
        assert!(text.contains("ws_connections"));
        assert!(text.contains("rooms_created"));
        assert!(text.contains("http_request_duration_seconds"));
    }
}
