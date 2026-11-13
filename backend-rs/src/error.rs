//! API error type with Go-compatible envelope mapping.
//!
//! Go responses: `{"status": "success"|"error", "message": "...", "result": ...}`
//! plus an HTTP status code. Handlers return `Result<ApiOk<T>, ApiError>` and a
//! wrapping layer serializes both branches into that envelope.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// One API failure: a client-visible variant plus an internal source. Serializes
/// to the Go envelope `{"status": "error", "message": ...}` with `status()` as
/// the HTTP code.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Unauthorized(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("internal error")]
    Internal(#[source] anyhow_no_backtrace::Error),
}

// Thin alias to keep the Internal variant's Display free of the anyhow preamble.
mod anyhow_no_backtrace {
    /// Boxed source without the anyhow preamble (keeps Display clean).
    pub type Error = Box<dyn std::error::Error + Send + Sync>;
}

impl ApiError {
    /// HTTP status for this variant (Internal → 500).
    pub fn status(&self) -> StatusCode {
        match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Client-visible message (Internal prefixes its source; never a raw backtrace).
    pub fn message(&self) -> String {
        match self {
            Self::Internal(source) => format!("internal error: {source}"),
            other => other.to_string(),
        }
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        tracing::error!(error = %e, "database error");
        Self::Internal(Box::new(e))
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status();
        // Go utils.Error parity: capitalize the first letter of the message.
        let mut message = self.message();
        if let Some(first) = message.get_mut(..1) {
            first.make_ascii_uppercase();
        }
        let body = json!({
            "status": "error",
            "message": message,
        });
        (status, axum::Json(body)).into_response()
    }
}

/// Handler return type: `Ok(ApiOk<T>)` or an [`ApiError`].
pub type ApiResult<T> = Result<T, ApiError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_codes_match_variants() {
        assert_eq!(
            ApiError::BadRequest("x".into()).status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            ApiError::Unauthorized("x".into()).status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            ApiError::Forbidden("x".into()).status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            ApiError::NotFound("x".into()).status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            ApiError::Conflict("x".into()).status(),
            StatusCode::CONFLICT
        );
        let internal = ApiError::Internal(Box::new(std::io::Error::other("boom")));
        assert_eq!(internal.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(internal.message().contains("boom"));
    }

    #[test]
    fn message_passes_client_variants_through() {
        assert_eq!(ApiError::NotFound("gone".into()).message(), "gone");
    }
}
