use argon2::password_hash::{SaltString, rand_core::OsRng};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use axum::{Json, extract::State, http::StatusCode};
use opencade_protocol::Envelope;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

use crate::{
    authn::{AuthUser, generate_session_token, hash_token},
    error::AppError,
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    username: String,
    password: String,
    email: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    identifier: String,
    password: String,
}

#[derive(Debug, Serialize)]
struct PublicUser<'a> {
    id: String,
    username: &'a str,
    email: Option<&'a str>,
}

fn validate_username(username: &str) -> Result<(), AppError> {
    let expression = Regex::new(r"^[a-zA-Z0-9_]{3,32}$")
        .map_err(|_| AppError::Internal("username validator unavailable".into()))?;
    if expression.is_match(username) {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "username must be 3-32 characters using letters, numbers, or underscore".into(),
        ))
    }
}

fn validate_email(email: &str) -> Result<(), AppError> {
    let (local, domain) = email
        .split_once('@')
        .ok_or_else(|| AppError::BadRequest("email must contain a valid domain".into()))?;
    if !local.is_empty() && domain.contains('.') && email.len() <= 254 {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "email must contain a valid domain".into(),
        ))
    }
}

fn validate_password(password: &str) -> Result<(), AppError> {
    if (8..=128).contains(&password.len()) {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "password must contain between 8 and 128 characters".into(),
        ))
    }
}

fn database_error(error: sqlx::Error, operation: &'static str) -> AppError {
    if error
        .as_database_error()
        .and_then(|database| database.code())
        .as_deref()
        == Some("23505")
    {
        return AppError::Conflict("username or email already exists".into());
    }
    tracing::error!(%error, operation, "database operation failed");
    AppError::Internal("database operation failed".into())
}

pub async fn register(
    State(state): State<AppState>,
    Json(request): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<Envelope<Value>>), AppError> {
    validate_username(&request.username)?;
    validate_password(&request.password)?;
    if let Some(email) = request.email.as_deref() {
        validate_email(email)?;
    }
    let rate_key = format!("register:{}", request.username.trim().to_ascii_lowercase());
    if !state.auth_rate_limiter.check(&rate_key)
        || !state
            .auth_rate_limiter
            .check_with_limit("register-global", 30)
    {
        return Err(AppError::RateLimited(
            "too many account creation attempts; retry in one minute".into(),
        ));
    }

    let salt = SaltString::generate(&mut OsRng);
    let password_hash = Argon2::default()
        .hash_password(request.password.as_bytes(), &salt)
        .map_err(|_| AppError::Internal("password hashing failed".into()))?
        .to_string();
    let token = generate_session_token();
    let token_hash = hash_token(&token);
    let expires_at = chrono::Utc::now() + chrono::Duration::days(30);

    let mut transaction = state
        .pool
        .begin()
        .await
        .map_err(|error| database_error(error, "begin registration transaction"))?;
    let user_id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (username, email, password_hash)
         VALUES ($1, $2, $3)
         RETURNING id",
    )
    .bind(&request.username)
    .bind(&request.email)
    .bind(password_hash)
    .fetch_one(&mut *transaction)
    .await
    .map_err(|error| database_error(error, "insert user"))?;
    sqlx::query("INSERT INTO sessions (user_id, token_hash, expires_at) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind(token_hash)
        .bind(expires_at)
        .execute(&mut *transaction)
        .await
        .map_err(|error| database_error(error, "insert session"))?;
    transaction
        .commit()
        .await
        .map_err(|error| database_error(error, "commit registration"))?;
    state.auth_rate_limiter.clear(&rate_key);

    let user = PublicUser {
        id: user_id.to_string(),
        username: &request.username,
        email: request.email.as_deref(),
    };
    Ok((
        StatusCode::CREATED,
        Json(Envelope::new(
            "auth.registered",
            json!({ "user": user, "token": token, "expires_at": expires_at }),
        )),
    ))
}

pub async fn login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<Envelope<Value>>, AppError> {
    validate_password(&request.password)?;
    if request.identifier.trim().is_empty() {
        return Err(AppError::BadRequest("identifier must not be empty".into()));
    }
    let rate_key = format!("login:{}", request.identifier.trim().to_ascii_lowercase());
    if !state.auth_rate_limiter.check(&rate_key)
        || !state
            .auth_rate_limiter
            .check_with_limit("login-global", 120)
    {
        return Err(AppError::RateLimited(
            "too many sign-in attempts; retry in one minute".into(),
        ));
    }

    let row = sqlx::query(
        "SELECT id, username, email, password_hash
         FROM users
         WHERE lower(username) = lower($1) OR lower(email) = lower($1)
         LIMIT 1",
    )
    .bind(request.identifier.trim())
    .fetch_optional(&state.pool)
    .await
    .map_err(|error| database_error(error, "find login user"))?;

    let Some(row) = row else {
        let salt = SaltString::encode_b64(b"opencade-dummy-salt")
            .map_err(|_| AppError::Internal("password verifier unavailable".into()))?;
        Argon2::default()
            .hash_password(request.password.as_bytes(), &salt)
            .map_err(|_| AppError::Internal("password verifier unavailable".into()))?;
        return Err(AppError::Unauthorized("invalid credentials".into()));
    };

    let user_id: Uuid = row
        .try_get("id")
        .map_err(|_| AppError::Internal("invalid user record".into()))?;
    let username: String = row
        .try_get("username")
        .map_err(|_| AppError::Internal("invalid user record".into()))?;
    let email: Option<String> = row
        .try_get("email")
        .map_err(|_| AppError::Internal("invalid user record".into()))?;
    let stored_hash: String = row
        .try_get("password_hash")
        .map_err(|_| AppError::Internal("invalid user record".into()))?;
    let parsed_hash = PasswordHash::new(&stored_hash)
        .map_err(|_| AppError::Internal("invalid password record".into()))?;
    Argon2::default()
        .verify_password(request.password.as_bytes(), &parsed_hash)
        .map_err(|_| AppError::Unauthorized("invalid credentials".into()))?;
    state.auth_rate_limiter.clear(&rate_key);

    let token = generate_session_token();
    let expires_at = chrono::Utc::now() + chrono::Duration::days(30);
    sqlx::query("INSERT INTO sessions (user_id, token_hash, expires_at) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind(hash_token(&token))
        .bind(expires_at)
        .execute(&state.pool)
        .await
        .map_err(|error| database_error(error, "insert login session"))?;

    let user = PublicUser {
        id: user_id.to_string(),
        username: &username,
        email: email.as_deref(),
    };
    Ok(Json(Envelope::new(
        "auth.logged_in",
        json!({ "user": user, "token": token, "expires_at": expires_at }),
    )))
}

pub async fn logout(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<Json<Envelope<Value>>, AppError> {
    sqlx::query("UPDATE sessions SET revoked_at = now() WHERE token_hash = $1")
        .bind(user.token_hash)
        .execute(&state.pool)
        .await
        .map_err(|error| database_error(error, "revoke session"))?;
    Ok(Json(Envelope::new(
        "auth.logged_out",
        json!({ "message": "session revoked" }),
    )))
}

pub async fn me(user: AuthUser) -> Json<Envelope<Value>> {
    Json(Envelope::new(
        "auth.me",
        json!({ "user": { "id": user.id, "username": user.username } }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_user_facing_boundaries() {
        assert!(validate_username("player_one").is_ok());
        assert!(validate_username("x").is_err());
        assert!(validate_email("player@example.com").is_ok());
        assert!(validate_email("player@example").is_err());
        assert!(validate_password("eight888").is_ok());
        assert!(validate_password("short").is_err());
        assert!(validate_password(&"x".repeat(129)).is_err());
    }
}
