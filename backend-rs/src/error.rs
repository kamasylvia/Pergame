//! API error type with Go-compatible envelope mapping.
//!
//! Go responses: `{"status": "success"|"error", "message": "...", "result": ...}`
//! plus an HTTP status code. Handlers return `Result<ApiOk<T>, ApiError>` and a
//! wrapping layer serializes both branches into that envelope.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

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
    pub type Error = Box<dyn std::error::Error + Send + Sync>;
}

impl ApiError {
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

pub type ApiResult<T> = Result<T, ApiError>;
