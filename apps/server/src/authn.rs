use argon2::password_hash::rand_core::{OsRng, RngCore};
use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts},
};
use sha2::{Digest, Sha256};
use sqlx::Row;
use uuid::Uuid;

use crate::{error::AppError, state::AppState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthUser {
    pub id: Uuid,
    pub username: String,
    pub token_hash: String,
}

pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn generate_session_token() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut token = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        token.push(HEX[usize::from(byte >> 4)] as char);
        token.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    token
}

pub async fn authenticate_token(state: &AppState, token: &str) -> Result<AuthUser, AppError> {
    if token.len() != 64 {
        return Err(AppError::Unauthorized("invalid session".into()));
    }
    let token_hash = hash_token(token);
    let row = sqlx::query(
        "SELECT users.id, users.username
         FROM sessions
         JOIN users ON users.id = sessions.user_id
         WHERE sessions.token_hash = $1
           AND sessions.revoked_at IS NULL
           AND sessions.expires_at > now()",
    )
    .bind(&token_hash)
    .fetch_optional(&state.pool)
    .await
    .map_err(|error| {
        tracing::error!(%error, "session lookup failed");
        AppError::Internal("database operation failed".into())
    })?
    .ok_or_else(|| AppError::Unauthorized("invalid or expired session".into()))?;

    Ok(AuthUser {
        id: row
            .try_get("id")
            .map_err(|_| AppError::Internal("invalid user record".into()))?,
        username: row
            .try_get("username")
            .map_err(|_| AppError::Internal("invalid user record".into()))?,
        token_hash,
    })
}

fn bearer_token(parts: &Parts) -> Result<&str, AppError> {
    let value = parts
        .headers
        .get(header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok())
        .ok_or_else(|| AppError::Unauthorized("missing bearer token".into()))?;
    value
        .strip_prefix("Bearer ")
        .filter(|token| !token.is_empty())
        .ok_or_else(|| AppError::Unauthorized("missing bearer token".into()))
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        authenticate_token(state, bearer_token(parts)?).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_tokens_have_256_bits_encoded_as_hex() {
        let first = generate_session_token();
        let second = generate_session_token();
        assert_eq!(first.len(), 64);
        assert!(first.chars().all(|character| character.is_ascii_hexdigit()));
        assert_ne!(first, second);
    }

    #[test]
    fn token_hash_is_stable_and_does_not_reveal_token() {
        let token = "a".repeat(64);
        let first = hash_token(&token);
        assert_eq!(first, hash_token(&token));
        assert_ne!(first, token);
    }
}
