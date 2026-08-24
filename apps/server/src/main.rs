use opencade_server::{AppState, Config, build_app, shutdown_signal};
use sqlx::postgres::PgPoolOptions;
use std::net::SocketAddr;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(config.rust_log.clone()));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .init();

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await?;

    if std::env::args().any(|arg| arg == "--migrate") {
        sqlx::migrate!("./migrations").run(&pool).await?;
        info!("database migrations completed");
        return Ok(());
    }

    sqlx::migrate!("./migrations").run(&pool).await?;

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(%addr, "opencade server listening");

    axum::serve(listener, build_app(AppState::new(pool, config)))
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}
