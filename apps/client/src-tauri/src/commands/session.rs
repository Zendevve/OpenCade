const SERVICE: &str = "com.opencade.client";
const ACCOUNT: &str = "session-token";

fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, ACCOUNT)
        .map_err(|_| "operating system credential storage is unavailable".to_string())
}

fn validate_token(token: &str) -> Result<(), String> {
    if token.len() < 32 || token.len() > 512 || token.chars().any(char::is_whitespace) {
        return Err("session token is invalid".into());
    }
    Ok(())
}

#[tauri::command]
pub fn store_session_token(token: String) -> Result<(), String> {
    validate_token(&token)?;
    entry()?
        .set_password(&token)
        .map_err(|_| "could not save the session in operating system credential storage".into())
}

#[tauri::command]
pub fn load_session_token() -> Result<Option<String>, String> {
    match entry()?.get_password() {
        Ok(token) => {
            validate_token(&token)?;
            Ok(Some(token))
        }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err("could not read the session from operating system credential storage".into()),
    }
}

#[tauri::command]
pub fn clear_session_token() -> Result<(), String> {
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => {
            Err("could not clear the session from operating system credential storage".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_tokens_without_exposing_them_to_logs() {
        assert!(validate_token(&"a".repeat(32)).is_ok());
        assert!(validate_token("too-short").is_err());
        assert!(validate_token(&format!("{} ", "a".repeat(32))).is_err());
    }
}
