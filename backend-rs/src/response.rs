//! Go-compatible response envelope:
//! `{"status": "success"|"error", "timestamp": <ms>, "result"|"message": ...}`

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

use crate::auth::service::now_ms;

/// Success envelope wrapper: handlers return `ApiOk { result, status }`.
pub struct ApiOk<T: Serialize> {
    pub result: T,
    pub status: StatusCode,
}

impl<T: Serialize> ApiOk<T> {
    pub fn ok(result: T) -> Self {
        Self { result, status: StatusCode::OK }
    }

    pub fn with_status(status: StatusCode, result: T) -> Self {
        Self { result, status }
    }
}

impl<T: Serialize> IntoResponse for ApiOk<T> {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(serde_json::json!({
                "status": "success",
                "timestamp": now_ms(),
                "result": serde_json::to_value(&self.result).unwrap_or(serde_json::Value::Null),
            })),
        )
            .into_response()
    }
}
