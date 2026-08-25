use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct RuntimeConfig {
    api_url: String,
    stun_server: Option<String>,
}

#[tauri::command]
pub fn runtime_config() -> Result<RuntimeConfig, String> {
    let api_url =
        std::env::var("OPENCADE_API_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
    validate_api_url(&api_url)?;
    let stun_server = std::env::var("OPENCADE_STUN_SERVER")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    if let Some(server) = &stun_server {
        server
            .parse::<std::net::SocketAddr>()
            .map_err(|_| "OPENCADE_STUN_SERVER must be a numeric IP:port".to_string())?;
    }
    Ok(RuntimeConfig {
        api_url: api_url.trim_end_matches('/').to_owned(),
        stun_server,
    })
}

fn validate_api_url(value: &str) -> Result<(), String> {
    let parsed = reqwest::Url::parse(value)
        .map_err(|_| "OPENCADE_API_URL must be a valid http(s) URL".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err("OPENCADE_API_URL must be a credential-free http(s) origin".into());
    }
    if parsed.scheme() == "http"
        && !matches!(parsed.host_str(), Some("localhost" | "127.0.0.1" | "::1"))
    {
        return Err("remote OPENCADE_API_URL values must use HTTPS".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_http_origins_and_rejects_other_schemes() {
        assert!(validate_api_url("https://alpha.example").is_ok());
        assert!(validate_api_url("http://127.0.0.1:8080").is_ok());
        assert!(validate_api_url("http://192.168.1.10:8080").is_err());
        assert!(validate_api_url("file:///tmp/server").is_err());
        assert!(validate_api_url("https://bad host").is_err());
    }
}
