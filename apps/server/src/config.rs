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
        Self::from_lookup(|key| env::var(key).ok())
    }

    fn from_lookup(mut lookup: impl FnMut(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let database_url = lookup("DATABASE_URL")
            .unwrap_or_else(|| "postgres://opencade:opencade@localhost:5432/opencade".to_string());
        let session_secret =
            lookup("SESSION_SECRET").unwrap_or_else(|| "dev-session-secret-change-me".to_string());
        let rust_log = lookup("RUST_LOG").unwrap_or_else(|| "info".to_string());
        let port = match lookup("PORT") {
            Some(value) => value
                .parse::<u16>()
                .ok()
                .filter(|port| *port > 0)
                .ok_or(ConfigError::InvalidPort)?,
            None => 8080,
        };
        let production =
            lookup("OPENCADE_ENV").is_some_and(|value| value.eq_ignore_ascii_case("production"));
        if production && session_secret.len() < 32 {
            return Err(ConfigError::WeakProductionSecret);
        }

        let allowed_origins = lookup("ALLOWED_ORIGINS")
            .unwrap_or_else(|| "http://localhost:1420,tauri://localhost".to_string())
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
    use std::collections::HashMap;

    fn config(values: &[(&str, &str)]) -> Result<Config, ConfigError> {
        let values = values.iter().copied().collect::<HashMap<_, _>>();
        Config::from_lookup(|key| values.get(key).map(|value| (*value).to_string()))
    }

    #[test]
    fn defaults_are_safe_for_local_development() {
        let config = config(&[]).expect("development defaults should be valid");
        assert_eq!(config.port, 8080);
        assert!(!config.production);
        assert!(config.allowed_origins.contains(&"tauri://localhost".into()));
    }

    #[test]
    fn rejects_invalid_port_instead_of_hiding_configuration_error() {
        assert_eq!(
            config(&[("PORT", "invalid")]),
            Err(ConfigError::InvalidPort)
        );
    }

    #[test]
    fn rejects_weak_production_secret() {
        assert_eq!(
            config(&[("OPENCADE_ENV", "production"), ("SESSION_SECRET", "weak")]),
            Err(ConfigError::WeakProductionSecret)
        );
    }

    #[test]
    fn parses_explicit_origins() {
        let config = config(&[(
            "ALLOWED_ORIGINS",
            "https://one.example, https://two.example",
        )])
        .expect("explicit origins should parse");
        assert_eq!(
            config.allowed_origins,
            vec!["https://one.example", "https://two.example"]
        );
    }
}
