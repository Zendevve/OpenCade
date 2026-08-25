//! Central error type for the server — maps domain errors to HTTP
//! responses using the versioned [`Envelope`] format.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use opencade_protocol::Envelope;
use serde_json::{Value, json};

/// Application-level errors.
///
/// Each variant maps to an HTTP status and a JSON envelope whose
/// payload is `{ "code": "<snake_case>", "message": "<detail>" }`.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("unauthorized: {0}")]
    Unauthorized(String),

    #[error("forbidden: {0}")]
    Forbidden(String),

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("internal error: {0}")]
    Internal(String),

    #[error("version unsupported: {0}")]
    VersionUnsupported(String),

    #[error("rate limited: {0}")]
    RateLimited(String),
}

impl AppError {
    /// HTTP status code for this error.
    fn status_code(&self) -> StatusCode {
        match self {
            AppError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            AppError::Forbidden(_) => StatusCode::FORBIDDEN,
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::VersionUnsupported(_) => StatusCode::BAD_REQUEST,
            AppError::RateLimited(_) => StatusCode::TOO_MANY_REQUESTS,
        }
    }

    /// Machine-readable code (snake_case) for the payload.
    fn code(&self) -> &'static str {
        match self {
            AppError::Unauthorized(_) => "unauthorized",
            AppError::Forbidden(_) => "forbidden",
            AppError::BadRequest(_) => "bad_request",
            AppError::NotFound(_) => "not_found",
            AppError::Conflict(_) => "conflict",
            AppError::Internal(_) => "internal",
            AppError::VersionUnsupported(_) => "version_unsupported",
            AppError::RateLimited(_) => "rate_limited",
        }
    }

    /// Human-readable message.
    fn message(&self) -> String {
        match self {
            AppError::Unauthorized(m)
            | AppError::Forbidden(m)
            | AppError::BadRequest(m)
            | AppError::NotFound(m)
            | AppError::Conflict(m)
            | AppError::Internal(m)
            | AppError::VersionUnsupported(m)
            | AppError::RateLimited(m) => m.clone(),
        }
    }

    /// Envelope type string for error responses.
    fn envelope_type(&self) -> String {
        format!("error.{}", self.code())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let code = self.code().to_string();
        let message = self.message();
        let envelope_type = self.envelope_type();

        let payload = json!({
            "code": code,
            "message": message,
        });

        // Envelope::new sets version, request_id, timestamp automatically.
        let envelope: Envelope<Value> = Envelope::new(envelope_type, payload);

        (status, Json(envelope)).into_response()
    }
}

// Convenience conversions

impl From<sqlx::Error> for AppError {
    fn from(_err: sqlx::Error) -> Self {
        AppError::Internal("database operation failed".to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::BadRequest(format!("invalid json: {}", err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    #[test]
    fn status_mapping() {
        assert_eq!(
            AppError::Unauthorized("x".into()).status_code(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            AppError::BadRequest("x".into()).status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            AppError::Forbidden("x".into()).status_code(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            AppError::NotFound("x".into()).status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            AppError::Internal("x".into()).status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            AppError::VersionUnsupported("x".into()).status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            AppError::RateLimited("x".into()).status_code(),
            StatusCode::TOO_MANY_REQUESTS
        );
    }

    #[test]
    fn code_mapping() {
        assert_eq!(AppError::Unauthorized("x".into()).code(), "unauthorized");
        assert_eq!(AppError::Forbidden("x".into()).code(), "forbidden");
        assert_eq!(AppError::BadRequest("x".into()).code(), "bad_request");
        assert_eq!(AppError::NotFound("x".into()).code(), "not_found");
        assert_eq!(AppError::Internal("x".into()).code(), "internal");
        assert_eq!(
            AppError::VersionUnsupported("x".into()).code(),
            "version_unsupported"
        );
    }

    #[test]
    fn display_contains_message() {
        let e = AppError::BadRequest("missing field".into());
        assert!(e.to_string().contains("missing field"));
    }
}
