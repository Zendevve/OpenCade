pub mod auth_rate_limit;
pub mod authn;
pub mod config;
pub mod error;
pub mod health;
pub mod metrics;
pub mod room_state;
pub mod routes;
pub mod state;
pub mod ws;

use axum::{
    Router,
    http::{HeaderValue, Method, header},
    routing::{get, post},
};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    trace::TraceLayer,
};

pub use config::Config;
pub use state::AppState;

pub fn build_app(state: AppState) -> Router {
    let cors = cors_layer(&state.config);

    Router::new()
        .route("/health", get(health::health))
        .route("/ready", get(health::ready))
        .route("/metrics", get(metrics::metrics))
        .route("/api/v1/auth/register", post(routes::auth::register))
        .route("/api/v1/auth/login", post(routes::auth::login))
        .route("/api/v1/auth/logout", post(routes::auth::logout))
        .route("/api/v1/auth/me", get(routes::auth::me))
        .route("/api/v1/games", get(routes::games::list_games))
        .route("/api/v1/games/{id}", get(routes::games::get_game))
        .route("/api/v1/servers", get(routes::servers::list_servers))
        .route(
            "/api/v1/challenges",
            get(routes::challenges::list_incoming).post(routes::challenges::create_challenge),
        )
        .route(
            "/api/v1/challenges/{id}/accept",
            post(routes::challenges::accept_challenge),
        )
        .route(
            "/api/v1/challenges/{id}/decline",
            post(routes::challenges::decline_challenge),
        )
        .route(
            "/api/v1/challenges/{id}/cancel",
            post(routes::challenges::cancel_challenge),
        )
        .route("/api/v1/lobbies/{game_id}", get(routes::lobbies::get_lobby))
        .route("/api/v1/rooms", post(routes::rooms::create_room))
        .route("/api/v1/rooms/{id}", get(routes::rooms::get_room))
        .route(
            "/api/v1/rooms/{id}/accept",
            post(routes::rooms::accept_room),
        )
        .route(
            "/api/v1/rooms/{id}/decline",
            post(routes::rooms::decline_room),
        )
        .route(
            "/api/v1/rooms/{id}/cancel",
            post(routes::rooms::cancel_room),
        )
        .route("/api/v1/rooms/{id}/start", post(routes::rooms::start_room))
        .route(
            "/api/v1/rooms/{id}/relay-ticket",
            post(routes::rooms::relay_ticket),
        )
        .route(
            "/api/v1/rooms/{id}/launch-grant",
            post(routes::rooms::create_launch_grant),
        )
        .route(
            "/api/v1/match-launch-grants/consume",
            post(routes::rooms::consume_launch_grant),
        )
        .route(
            "/api/v1/rooms/{id}/finish",
            post(routes::rooms::finish_room),
        )
        .route("/ws", get(ws::ws_handler))
        .with_state(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
}

fn cors_layer(config: &Config) -> CorsLayer {
    let origins = config
        .allowed_origins
        .iter()
        .filter_map(|origin| origin.parse::<HeaderValue>().ok())
        .collect::<Vec<_>>();

    CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
}

pub async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!(%error, "failed to install Ctrl+C handler");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => {
                tracing::error!(%error, "failed to install terminate handler");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use sqlx::postgres::PgPoolOptions;
    use tower::ServiceExt;

    #[test]
    fn configured_cors_does_not_allow_arbitrary_origins() {
        let config = Config::for_test();
        let _layer = cors_layer(&config);
        assert_eq!(config.allowed_origins, vec!["http://localhost:1420"]);
    }

    #[tokio::test]
    async fn health_is_available_without_a_database_round_trip() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://opencade:opencade@localhost/opencade_test")
            .expect("valid test database URL");
        let response = build_app(AppState::new(pool, Config::for_test()))
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("health response");
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn protected_routes_reject_missing_session_before_database_access() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://opencade:opencade@localhost/opencade_test")
            .expect("valid test database URL");
        let response = build_app(AppState::new(pool, Config::for_test()))
            .oneshot(
                Request::builder()
                    .uri("/api/v1/games")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("unauthorized response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
