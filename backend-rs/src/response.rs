//! Go-compatible response envelope:
//! `{"status": "success"|"error", "timestamp": <ms>, "result"|"message": ...}`

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use crate::auth::service::now_ms;

/// Success envelope wrapper: handlers return `ApiOk { result, status }`.
pub struct ApiOk<T: Serialize> {
    pub result: T,
    pub status: StatusCode,
}

impl<T: Serialize> ApiOk<T> {
    /// 200 envelope around a serializable result.
    pub fn ok(result: T) -> Self {
        Self {
            result,
            status: StatusCode::OK,
        }
    }

    /// Envelope with an explicit status (201, …).
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn envelope_shape() {
        let resp = ApiOk::ok(serde_json::json!({"a": 1})).into_response();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["status"], "success");
        assert_eq!(v["result"]["a"], 1);
        assert!(v["timestamp"].as_i64().unwrap() > 0);

        let created =
            ApiOk::with_status(StatusCode::CREATED, serde_json::json!(null)).into_response();
        assert_eq!(created.status(), StatusCode::CREATED);
    }
}
