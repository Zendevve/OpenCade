use std::env;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("PORT must be an integer between 1 and 65535")]
    InvalidPort,
    #[error("SESSION_SECRET must contain at least 32 characters in production")]
    WeakProductionSecret,
    #[error("ALLOWED_ORIGINS must contain at least one origin")]
    MissingAllowedOrigins,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub database_url: String,
    pub session_secret: String,
    pub rust_log: String,
    pub port: u16,
    pub production: bool,
    pub allowed_origins: Vec<String>,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let database_url = env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://opencade:opencade@localhost:5432/opencade".to_string());
        let session_secret = env::var("SESSION_SECRET")
            .unwrap_or_else(|_| "dev-session-secret-change-me".to_string());
        let rust_log = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
        let port = match env::var("PORT") {
            Ok(value) => value
                .parse::<u16>()
                .ok()
                .filter(|port| *port > 0)
                .ok_or(ConfigError::InvalidPort)?,
            Err(_) => 8080,
        };
        let production = env::var("OPENCADE_ENV")
            .map(|value| value.eq_ignore_ascii_case("production"))
            .unwrap_or(false);
        if production && session_secret.len() < 32 {
            return Err(ConfigError::WeakProductionSecret);
        }

        let allowed_origins = env::var("ALLOWED_ORIGINS")
            .unwrap_or_else(|_| "http://localhost:1420,tauri://localhost".to_string())
            .split(',')
            .map(str::trim)
            .filter(|origin| !origin.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        if allowed_origins.is_empty() {
            return Err(ConfigError::MissingAllowedOrigins);
        }

        Ok(Self {
            database_url,
            session_secret,
            rust_log,
            port,
            production,
            allowed_origins,
        })
    }

    pub fn for_test() -> Self {
        Self {
            database_url: "postgres://opencade:opencade@localhost:5432/opencade_test".into(),
            session_secret: "test-session-secret-with-32-characters".into(),
            rust_log: "info".into(),
            port: 8080,
            production: false,
            allowed_origins: vec!["http://localhost:1420".into()],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    const KEYS: [&str; 6] = [
        "DATABASE_URL",
        "SESSION_SECRET",
        "RUST_LOG",
        "PORT",
        "OPENCADE_ENV",
        "ALLOWED_ORIGINS",
    ];

    fn clear_env() {
        for key in KEYS {
            env::remove_var(key);
        }
    }

    #[test]
    #[serial]
    fn defaults_are_safe_for_local_development() {
        clear_env();
        let config = Config::from_env().expect("development defaults should be valid");
        assert_eq!(config.port, 8080);
        assert!(!config.production);
        assert!(config.allowed_origins.contains(&"tauri://localhost".into()));
        clear_env();
    }

    #[test]
    #[serial]
    fn rejects_invalid_port_instead_of_hiding_configuration_error() {
        clear_env();
        env::set_var("PORT", "invalid");
        assert_eq!(Config::from_env(), Err(ConfigError::InvalidPort));
        clear_env();
    }

    #[test]
    #[serial]
    fn rejects_weak_production_secret() {
        clear_env();
        env::set_var("OPENCADE_ENV", "production");
        env::set_var("SESSION_SECRET", "weak");
        assert_eq!(Config::from_env(), Err(ConfigError::WeakProductionSecret));
        clear_env();
    }

    #[test]
    #[serial]
    fn parses_explicit_origins() {
        clear_env();
        env::set_var(
            "ALLOWED_ORIGINS",
            "https://one.example, https://two.example",
        );
        let config = Config::from_env().expect("explicit origins should parse");
        assert_eq!(
            config.allowed_origins,
            vec!["https://one.example", "https://two.example"]
        );
        clear_env();
    }
}
