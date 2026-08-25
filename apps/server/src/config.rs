use std::env;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("PORT must be an integer between 1 and 65535")]
    InvalidPort,
    #[error("STUN_PORT must be an integer between 1 and 65535")]
    InvalidStunPort,
    #[error("SESSION_SECRET must contain at least 32 characters in production")]
    WeakProductionSecret,
    #[error("ALLOWED_ORIGINS must contain at least one origin")]
    MissingAllowedOrigins,
    #[error("RELAY_URL and RELAY_AUTH_SECRET must be configured together")]
    IncompleteRelayConfig,
    #[error("RELAY_AUTH_SECRET must contain at least 32 characters")]
    WeakRelaySecret,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Config {
    pub database_url: String,
    pub session_secret: String,
    pub rust_log: String,
    pub port: u16,
    pub production: bool,
    pub allowed_origins: Vec<String>,
    pub stun_host: String,
    pub stun_port: u16,
    pub relay_url: Option<String>,
    pub relay_secret: Option<String>,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Config")
            .field("database_url", &"<redacted>")
            .field("session_secret", &"<redacted>")
            .field("rust_log", &self.rust_log)
            .field("port", &self.port)
            .field("production", &self.production)
            .field("allowed_origins", &self.allowed_origins)
            .field("stun_host", &self.stun_host)
            .field("stun_port", &self.stun_port)
            .field("relay_url", &self.relay_url)
            .field(
                "relay_secret",
                &self.relay_secret.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
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
            .unwrap_or_else(|| {
                "http://localhost:1420,tauri://localhost,http://tauri.localhost,https://tauri.localhost"
                    .to_string()
            })
            .split(',')
            .map(str::trim)
            .filter(|origin| !origin.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        if allowed_origins.is_empty() {
            return Err(ConfigError::MissingAllowedOrigins);
        }

        let stun_host = lookup("STUN_HOST").unwrap_or_else(|| "stun.opencade.local".to_string());
        let stun_port = match lookup("STUN_PORT") {
            Some(value) => value
                .parse::<u16>()
                .ok()
                .filter(|p| *p > 0)
                .ok_or(ConfigError::InvalidStunPort)?,
            None => 3478,
        };
        let relay_url = lookup("RELAY_URL")
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty());
        let relay_secret = lookup("RELAY_AUTH_SECRET")
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty());
        if relay_url.is_some() != relay_secret.is_some() {
            return Err(ConfigError::IncompleteRelayConfig);
        }
        if relay_secret
            .as_ref()
            .is_some_and(|secret| secret.len() < 32)
        {
            return Err(ConfigError::WeakRelaySecret);
        }

        Ok(Self {
            database_url,
            session_secret,
            rust_log,
            port,
            production,
            allowed_origins,
            stun_host,
            stun_port,
            relay_url,
            relay_secret,
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
            stun_host: "127.0.0.1".into(),
            stun_port: 3478,
            relay_url: None,
            relay_secret: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn load_config(values: &[(&str, &str)]) -> Result<Config, ConfigError> {
        let values = values.iter().copied().collect::<HashMap<_, _>>();
        Config::from_lookup(|key| values.get(key).map(|value| (*value).to_string()))
    }

    #[test]
    fn defaults_are_safe_for_local_development() {
        let config = load_config(&[]).expect("development defaults should be valid");
        assert_eq!(config.port, 8080);
        assert!(!config.production);
        assert!(config.allowed_origins.contains(&"tauri://localhost".into()));
        assert!(
            config
                .allowed_origins
                .contains(&"https://tauri.localhost".into())
        );
        assert_eq!(config.stun_host, "stun.opencade.local");
        assert_eq!(config.stun_port, 3478);
        assert_eq!(config.relay_url, None);
    }

    #[test]
    fn for_test_uses_deterministic_stun_defaults() {
        let config = Config::for_test();
        assert_eq!(config.stun_host, "127.0.0.1");
        assert_eq!(config.stun_port, 3478);
        assert_eq!(config.relay_url, None);
    }

    #[test]
    fn rejects_invalid_port_instead_of_hiding_configuration_error() {
        assert_eq!(
            load_config(&[("PORT", "invalid")]),
            Err(ConfigError::InvalidPort)
        );
    }

    #[test]
    fn rejects_invalid_stun_port() {
        assert_eq!(
            load_config(&[("STUN_PORT", "not_a_port")]),
            Err(ConfigError::InvalidStunPort)
        );
        assert_eq!(
            load_config(&[("STUN_PORT", "0")]),
            Err(ConfigError::InvalidStunPort)
        );
    }

    #[test]
    fn rejects_weak_production_secret() {
        assert_eq!(
            load_config(&[("OPENCADE_ENV", "production"), ("SESSION_SECRET", "weak")]),
            Err(ConfigError::WeakProductionSecret)
        );
    }

    #[test]
    fn parses_explicit_origins() {
        let config = load_config(&[(
            "ALLOWED_ORIGINS",
            "https://one.example, https://two.example",
        )])
        .expect("explicit origins should parse");
        assert_eq!(
            config.allowed_origins,
            vec!["https://one.example", "https://two.example"]
        );
    }

    #[test]
    fn parses_stun_and_relay_env() {
        let config = load_config(&[
            ("STUN_HOST", "stun.example.com"),
            ("STUN_PORT", "3479"),
            ("RELAY_URL", "wss://relay.example.com/relay"),
            (
                "RELAY_AUTH_SECRET",
                "relay-test-secret-at-least-32-bytes-long",
            ),
        ])
        .expect("stun and relay should parse");
        assert_eq!(config.stun_host, "stun.example.com");
        assert_eq!(config.stun_port, 3479);
        assert_eq!(
            config.relay_url,
            Some("wss://relay.example.com/relay".into())
        );
    }

    #[test]
    fn relay_url_absent_when_not_set_or_empty() {
        let config = load_config(&[]).expect("defaults");
        assert_eq!(config.relay_url, None);
        assert_eq!(config.relay_secret, None);
        let config = load_config(&[("RELAY_URL", "")]).expect("empty relay should be None");
        assert_eq!(config.relay_url, None);
        let config = load_config(&[("RELAY_URL", "   ")]).expect("whitespace relay should be None");
        assert_eq!(config.relay_url, None);
        assert_eq!(config.relay_secret, None);
    }

    #[test]
    fn rejects_incomplete_or_weak_relay_configuration() {
        assert_eq!(
            load_config(&[("RELAY_URL", "wss://relay.example.com/relay")]),
            Err(ConfigError::IncompleteRelayConfig)
        );
        assert_eq!(
            load_config(&[
                ("RELAY_URL", "wss://relay.example.com/relay"),
                ("RELAY_AUTH_SECRET", "weak"),
            ]),
            Err(ConfigError::WeakRelaySecret)
        );
    }
}
