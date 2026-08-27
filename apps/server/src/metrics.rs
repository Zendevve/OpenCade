use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use std::sync::atomic::Ordering;

use crate::{error::AppError, state::AppState};

/// Operator-authenticated Prometheus exposition for production monitoring.
pub async fn metrics(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    state.require_operator(
        headers
            .get("x-operator-token")
            .and_then(|value| value.to_str().ok()),
    )?;
    let rooms = state.metrics.rooms_created.load(Ordering::Relaxed);
    let http_total = state.metrics.http_requests_total.load(Ordering::Relaxed);
    let responses_4xx = state.metrics.http_responses_4xx.load(Ordering::Relaxed);
    let responses_5xx = state.metrics.http_responses_5xx.load(Ordering::Relaxed);
    let duration_seconds = state
        .metrics
        .http_request_duration_micros
        .load(Ordering::Relaxed) as f64
        / 1_000_000.0;
    let ws = state.ws_hub.len() as u64;

    let body = format!(
        "# HELP http_requests_total Total HTTP requests\n\
# TYPE http_requests_total counter\n\
http_requests_total {http_total}\n\
# HELP http_responses_total HTTP responses by status class\n\
# TYPE http_responses_total counter\n\
http_responses_total{{class=\"4xx\"}} {responses_4xx}\n\
http_responses_total{{class=\"5xx\"}} {responses_5xx}\n\
# HELP ws_connections Current WebSocket connections\n\
# TYPE ws_connections gauge\n\
ws_connections {ws}\n\
# HELP rooms_created Total rooms created\n\
# TYPE rooms_created counter\n\
rooms_created {rooms}\n\
# HELP http_request_duration_seconds Cumulative HTTP request duration\n\
# TYPE http_request_duration_seconds summary\n\
http_request_duration_seconds_sum {duration_seconds}\n\
http_request_duration_seconds_count {http_total}\n"
    );

    Ok((
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4",
        )],
        body,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::Config, state::AppState};
    use axum::{Router, body::Body, http::Request, routing::get};
    use tower::ServiceExt;

    #[tokio::test]
    async fn metrics_requires_operator_and_contains_real_counters() {
        let pool =
            sqlx::PgPool::connect_lazy("postgres://opencade:opencade@localhost:5432/opencade_test")
                .expect("lazy pool");
        let state = AppState::new(pool, Config::for_test());
        let app = Router::new()
            .route("/metrics", get(metrics))
            .with_state(state);
        let unauthorized = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .header("x-operator-token", "test-operator-token-with-32-characters")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), 16 * 1024)
            .await
            .unwrap();
        let text = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(text.contains("http_requests_total"));
        assert!(text.contains("http_responses_total"));
        assert!(text.contains("ws_connections"));
        assert!(text.contains("rooms_created"));
        assert!(text.contains("http_request_duration_seconds"));
    }
}
