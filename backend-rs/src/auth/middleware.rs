//! `AuthUser` extractor: verifies the `Authorization` cookie JWT (HS256,
//! Go middleware parity) and yields the acting user identity.

use axum::Json;
use axum::extract::FromRequestParts;
use axum::http::StatusCode;
use axum::http::request::Parts;
use serde_json::json;

use uuid::Uuid;

#[derive(Debug, Clone)]
/// Authenticated identity from the `Authorization` cookie JWT.
pub struct AuthUser {
    pub id: Uuid,
    pub role: i32,
}

impl AuthUser {
    /// The acting user's id.
    pub fn user_id(&self) -> Uuid {
        self.id
    }
}

fn unauthorized(message: &str) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({
            "status": "error",
            "timestamp": super::service::now_ms(),
            "message": message,
        })),
    )
}

impl axum::extract::OptionalFromRequestParts<crate::AppState> for AuthUser {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &crate::AppState,
    ) -> Result<Option<Self>, Self::Rejection> {
        // Infallible rejection => always Ok; invalid/missing cookie = None.
        Ok(
            <Self as FromRequestParts<crate::AppState>>::from_request_parts(parts, state)
                .await
                .ok(),
        )
    }
}

impl FromRequestParts<crate::AppState> for AuthUser {
    type Rejection = (StatusCode, Json<serde_json::Value>);

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &crate::AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(axum::http::header::COOKIE)
            .and_then(|v| v.to_str().ok())
            .and_then(|cookies| {
                cookies.split(';').find_map(|c| {
                    let c = c.trim();
                    c.strip_prefix("Authorization=")
                        .map(|v| v.trim().to_string())
                })
            });

        let Some(token) = token else {
            return Err(unauthorized("Bad access token."));
        };

        match super::jwt::verify_access_token(&token) {
            Ok((user_id, role)) => Ok(AuthUser { id: user_id, role }),
            Err(_) => Err(unauthorized("Bad access token.")),
        }
    }
}
