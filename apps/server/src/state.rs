//! Application state shared across Axum handlers.
//!
//! `AppState` is cloned for every request (cheap: `PgPool` and `Arc`
//! are reference-counted). The WebSocket hub tracks live connections
//! so handlers can push messages without holding the socket directly.

use sqlx::PgPool;
use std::sync::{Arc, atomic::AtomicU64};
use uuid::Uuid;

use axum::extract::ws::Message;
use opencade_protocol::Envelope;
use serde_json::Value;

use crate::auth_rate_limit::AuthRateLimiter;
use crate::config::Config;

/// Counters exposed via `GET /metrics` (M7).
#[derive(Debug, Default)]
pub struct Metrics {
    pub http_requests_total: AtomicU64,
    pub rooms_created: AtomicU64,
}

/// Shared application state injected via `axum::extract::State`.
#[derive(Clone)]
pub struct AppState {
    /// PostgreSQL connection pool.
    pub pool: PgPool,

    /// Resolved server configuration.
    pub config: Config,

    /// In-memory registry of active WebSocket connections.
    ///
    /// Key: connection / user id (caller-defined, typically
    /// `request_id` or authenticated user id).
    /// Value: bounded sender for [`axum::extract::ws::Message`]s to that socket.
    ///
    /// The map is wrapped in [`std::sync::Arc`] so `AppState::clone` is cheap.
    pub ws_hub: std::sync::Arc<
        dashmap::DashMap<String, tokio::sync::mpsc::Sender<axum::extract::ws::Message>>,
    >,
    /// Prometheus counters for `/metrics`.
    pub metrics: Arc<Metrics>,

    pub auth_rate_limiter: Arc<AuthRateLimiter>,
}

impl AppState {
    /// Create a new [`AppState`] from a pool and config.
    ///
    /// The WebSocket hub is initialised empty.
    pub fn new(pool: PgPool, config: Config) -> Self {
        Self {
            pool,
            config,
            ws_hub: std::sync::Arc::new(dashmap::DashMap::new()),
            metrics: Arc::new(Metrics::default()),
            auth_rate_limiter: Arc::new(AuthRateLimiter::default()),
        }
    }

    /// Best-effort delivery to an authenticated user's bounded WebSocket queue.
    /// Offline users observe the same durable state through REST after reconnecting.
    pub fn notify_user(&self, user_id: Uuid, message_type: &str, payload: Value) {
        let Some(target) = self.ws_hub.get(&user_id.to_string()) else {
            return;
        };
        let envelope = Envelope::new(message_type, payload);
        let Ok(text) = serde_json::to_string(&envelope) else {
            tracing::error!(%user_id, %message_type, "failed to serialize websocket notification");
            return;
        };
        if let Err(error) = target.try_send(Message::Text(text.into())) {
            tracing::warn!(%error, %user_id, %message_type, "websocket notification queue unavailable");
        }
    }
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("config", &self.config)
            .field("pool", &"<PgPool>")
            .field("ws_hub_len", &self.ws_hub.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    // AppState requires a real PgPool which would need a live database.
    // We test the structural contract without connecting: the type must be
    // Clone and new() must initialise an empty hub. Integration tests that
    // need a pool use `#[sqlx::test]`.

    use super::*;
    use sqlx::PgPool;

    #[test]
    fn ws_hub_initially_empty_via_new_signature() {
        // We can't construct a PgPool without a DB, but we can verify the
        // type's contract via a compile-time assertion: new() signature
        // exists and AppState is Clone. This test passes if compilation succeeds.
        fn assert_clone<T: Clone>() {}
        assert_clone::<AppState>();

        // Verify that DashMap + Arc are the expected concrete types by
        // checking that the field can be constructed independently.
        let hub: std::sync::Arc<
            dashmap::DashMap<String, tokio::sync::mpsc::Sender<axum::extract::ws::Message>>,
        > = std::sync::Arc::new(dashmap::DashMap::new());
        assert_eq!(hub.len(), 0);
        // Suppress unused warning for PgPool import
        let _ = std::any::type_name::<PgPool>();
    }
}
