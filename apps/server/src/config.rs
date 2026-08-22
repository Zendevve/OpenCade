//! Server configuration — loaded from environment variables.
//!
//! All values have sensible defaults for local development and CI.
//! In production every value should be supplied via the environment
//! or a `.env` file (loaded by `dotenvy` in `main.rs`).

use std::env;

/// Application configuration.
///
/// Constructed once at startup via [`Config::from_env`] and cloned
/// into [`crate::state::AppState`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// PostgreSQL connection string.
    ///
    /// Env var: `DATABASE_URL`
    /// Default: `postgres://openfight:openfight@db:5432/openfight`
    pub database_url: String,

    /// Secret used to sign/derive session tokens.
    ///
    /// Env var: `SESSION_SECRET`
    /// Default: `change-me` (must be overridden in production).
    pub session_secret: String,

    /// HTTP listen port.
    ///
    /// Env var: `PORT`
    /// Default: `8080`
    pub port: u16,

    /// `tracing` filter directive.
    ///
    /// Env var: `RUST_LOG`
    /// Default: `info`
    pub rust_log: String,
}

impl Config {
    /// Build [`Config`] from environment variables, falling back to
    /// documented defaults when a variable is absent or unparsable.
    ///
    /// # Defaults
    ///
    /// - `DATABASE_URL` → `postgres://openfight:openfight@db:5432/openfight`
    /// - `SESSION_SECRET` → `change-me`
    /// - `PORT` → `8080` (invalid integers also fall back to `8080`)
    /// - `RUST_LOG` → `info`
    pub fn from_env() -> Self {
        let database_url = env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://openfight:openfight@db:5432/openfight".to_string());

        let session_secret = env::var("SESSION_SECRET").unwrap_or_else(|_| "change-me".to_string());

        let rust_log = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());

        let port = env::var("PORT")
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(8080);

        Self {
            database_url,
            session_secret,
            port,
            rust_log,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::from_env()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;

    // Helper to run a test with a clean env, restoring afterwards would be
    // ideal but tests run with --test-threads=1 in CI; we take care to
    // remove keys we set so parallel runs do not leak.

    #[test]
    #[serial]
    fn defaults_when_env_missing() {
        // Ensure keys are absent
        env::remove_var("DATABASE_URL");
        env::remove_var("SESSION_SECRET");
        env::remove_var("PORT");
        env::remove_var("RUST_LOG");

        let cfg = Config::from_env();
        assert_eq!(
            cfg.database_url,
            "postgres://openfight:openfight@db:5432/openfight"
        );
        assert_eq!(cfg.session_secret, "change-me");
        assert_eq!(cfg.port, 8080);
        assert_eq!(cfg.rust_log, "info");
    }

    #[test]
    #[serial]
    fn reads_from_env() {
        env::set_var("DATABASE_URL", "postgres://user:pass@db:5432/testdb");
        env::set_var("SESSION_SECRET", "super-secret");
        env::set_var("PORT", "3001");
        env::set_var("RUST_LOG", "debug");

        let cfg = Config::from_env();
        assert_eq!(cfg.database_url, "postgres://user:pass@db:5432/testdb");
        assert_eq!(cfg.session_secret, "super-secret");
        assert_eq!(cfg.port, 3001);
        assert_eq!(cfg.rust_log, "debug");

        // cleanup
        env::remove_var("DATABASE_URL");
        env::remove_var("SESSION_SECRET");
        env::remove_var("PORT");
        env::remove_var("RUST_LOG");
    }

    #[test]
    #[serial]
    fn invalid_port_falls_back() {
        env::set_var("PORT", "not-a-number");
        // set other vars to defaults so test is isolated
        env::remove_var("DATABASE_URL");
        env::remove_var("SESSION_SECRET");
        env::remove_var("RUST_LOG");

        let cfg = Config::from_env();
        assert_eq!(cfg.port, 8080);

        env::remove_var("PORT");
    }

    #[test]
    #[serial]
    fn port_boundary_values() {
        env::set_var("PORT", "0");
        assert_eq!(Config::from_env().port, 0);
        env::set_var("PORT", "65535");
        assert_eq!(Config::from_env().port, 65535);
        env::remove_var("PORT");
    }
}
