use opencade_server::{AppState, Config, build_app, lifecycle, shutdown_signal};
use sqlx::postgres::PgPoolOptions;
use std::{net::SocketAddr, time::Duration};
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(config.rust_log.clone()));
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false);
    if config.production {
        subscriber.json().init();
    } else {
        subscriber.compact().init();
    }

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Duration::from_secs(10 * 60))
        .max_lifetime(Duration::from_secs(30 * 60))
        .connect(&config.database_url)
        .await?;

    if std::env::args().any(|arg| arg == "--migrate") {
        sqlx::migrate!("./migrations").run(&pool).await?;
        info!("database migrations completed");
        return Ok(());
    }

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(%addr, "opencade server listening");

    let state = AppState::new(pool, config);
    lifecycle::spawn_reconciler(state.clone());
    lifecycle::spawn_telemetry_retention(state.clone());
    axum::serve(listener, build_app(state))
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}
