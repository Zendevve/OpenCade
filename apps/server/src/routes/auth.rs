use argon2::password_hash::{rand_core::OsRng, SaltString};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use openfight_protocol::Envelope;
use regex::Regex;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tracing::info;
use uuid::Uuid;

use sqlx::Row;

use crate::error::AppError;
use crate::state::AppState;

fn validate_username(username: &str) -> Result<(), AppError> {
    let re = Regex::new(r"^[a-zA-Z0-9_]{3,32}$")
        .map_err(|e| AppError::Internal(format!("regex error: {}", e)))?;
    if !re.is_match(username) {
        return Err(AppError::BadRequest(
            "username must be 3-32 chars, alphanumeric or underscore".to_string(),
        ));
    }
    Ok(())
}

fn validate_email(email: &str) -> Result<(), AppError> {
    if !email.contains('@') {
        return Err(AppError::BadRequest("email must contain @".to_string()));
    }
    if email.len() < 3 || email.len() > 254 {
        return Err(AppError::BadRequest("email length invalid".to_string()));
    }
    Ok(())
}

fn validate_password(password: &str) -> Result<(), AppError> {
    if password.len() < 8 {
        return Err(AppError::BadRequest(
            "password must be at least 8 characters".to_string(),
        ));
    }
    Ok(())
}

fn hash_token_sha256(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// POST /api/v1/auth/register
/// Body: { username, email, password }
pub async fn register(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Result<impl IntoResponse, AppError> {
    let username = body
        .get("username")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("missing username".to_string()))?;
    let email = body
        .get("email")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("missing email".to_string()))?;
    let password = body
        .get("password")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("missing password".to_string()))?;

    validate_username(username)?;
    validate_email(email)?;
    validate_password(password)?;

    let salt = SaltString::generate(&mut OsRng);
    let password_hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| AppError::Internal(format!("hash error: {}", e)))?
        .to_string();

    let row = sqlx::query(
        "INSERT INTO users (username, email, password_hash) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(username)
    .bind(email)
    .bind(&password_hash)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        let msg = e.to_string();
        if msg.contains("duplicate") || msg.contains("unique") || msg.contains("already exists") {
            AppError::BadRequest("username or email already exists".to_string())
        } else {
            AppError::Internal(format!("database error: {}", e))
        }
    })?;

    let user_id: Uuid = row
        .try_get("id")
        .map_err(|e| AppError::Internal(format!("row decode error: {}", e)))?;

    info!(username = %username, user_id = %user_id, "auth: user registered");

    let payload = json!({
        "user_id": user_id.to_string(),
        "username": username,
        "email": email,
    });
    let envelope = Envelope::new("auth.registered", payload);
    Ok((StatusCode::CREATED, Json(envelope)))
}

/// POST /api/v1/auth/login
/// Body: { email, password }  (username may be used as fallback for email field)
pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Result<impl IntoResponse, AppError> {
    let email = body
        .get("email")
        .and_then(|v| v.as_str())
        .or_else(|| body.get("username").and_then(|v| v.as_str()))
        .ok_or_else(|| AppError::BadRequest("missing email".to_string()))?;
    let password = body
        .get("password")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("missing password".to_string()))?;

    validate_email(email)?;
    validate_password(password)?;

    let row = sqlx::query("SELECT id, username, email, password_hash FROM users WHERE email = $1")
        .bind(email)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => AppError::Unauthorized("invalid credentials".to_string()),
            other => AppError::Internal(format!("database error: {}", other)),
        })?;

    let user_id: Uuid = row
        .try_get("id")
        .map_err(|e| AppError::Internal(format!("row decode error: {}", e)))?;
    let stored_hash: String = row
        .try_get("password_hash")
        .map_err(|e| AppError::Internal(format!("row decode error: {}", e)))?;

    let parsed_hash = PasswordHash::new(&stored_hash)
        .map_err(|e| AppError::Internal(format!("hash parse error: {}", e)))?;

    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .map_err(|_| AppError::Unauthorized("invalid credentials".to_string()))?;

    let token = Uuid::new_v4().to_string();
    let token_hash = hash_token_sha256(&token);
    let expires_at = chrono::Utc::now() + chrono::Duration::days(7);

    sqlx::query(
        "INSERT INTO sessions (user_id, token_hash, expires_at) VALUES ($1, $2, $3)",
    )
    .bind(user_id)
    .bind(&token_hash)
    .bind(expires_at)
    .execute(&state.pool)
    .await
    .map_err(|e| AppError::Internal(format!("database error: {}", e)))?;

    info!(user_id = %user_id, "auth: user logged in");

    let payload = json!({
        "user_id": user_id.to_string(),
        "token": token,
    });
    let envelope = Envelope::new("auth.logged_in", payload);
    Ok((StatusCode::OK, Json(envelope)))
}

/// POST /api/v1/auth/logout
/// Body: { token }  — revokes session by token_hash
pub async fn logout(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Result<impl IntoResponse, AppError> {
    let token = body
        .get("token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("missing token".to_string()))?;

    if token.len() < 8 {
        return Err(AppError::BadRequest("invalid token".to_string()));
    }

    let token_hash = hash_token_sha256(token);

    let result = sqlx::query(
        "UPDATE sessions SET revoked_at = now() WHERE token_hash = $1 AND revoked_at IS NULL",
    )
    .bind(&token_hash)
    .execute(&state.pool)
    .await
    .map_err(|e| AppError::Internal(format!("database error: {}", e)))?;

    if result.rows_affected() == 0 {
        info!(token_hash = %token_hash, "auth: logout token not found or already revoked");
    } else {
        info!(token_hash = %token_hash, "auth: session revoked");
    }

    let payload = json!({ "message": "logout successful" });
    let envelope = Envelope::new("auth.logged_out", payload);
    Ok((StatusCode::OK, Json(envelope)))
}
