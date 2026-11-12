//! Session management + password-reset email flows (Go auth controller
//! GetSessions / DeleteSession / LogoutAllDevices / RequestResetPassword /
//! ResetPassword parity).

use axum::Json;
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::AppState;
use crate::auth::service::now_ms;
use crate::auth::{AuthUser, cookies};
use crate::error::ApiError;
use crate::repo::sessions as session_repo;
use crate::response::ApiOk;

/// GET /auth/sessions — all sessions of the caller.
pub async fn get_sessions(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<ApiOk<Vec<serde_json::Value>>, ApiError> {
    let rows = session_repo::list_by_user(&state.db, auth.user_id())
        .await
        .map_err(|_| {
            ApiError::Internal(Box::new(std::io::Error::other(
                "failed to retrieve sessions",
            )))
        })?;
    let value: Vec<serde_json::Value> = rows
        .iter()
        .map(|s| serde_json::to_value(s).unwrap_or(json!(null)))
        .collect();
    Ok(ApiOk::ok(value))
}

/// DELETE /auth/sessions/:sessionId — close one of the caller's sessions.
pub async fn delete_session(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(session_id): Path<String>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let id =
        Uuid::parse_str(&session_id).map_err(|_| ApiError::BadRequest("invalid parameter".into()))?;
    session_repo::delete_by_id_and_user(&state.db, id, auth.user_id())
        .await
        .map_err(|_| {
            ApiError::Internal(Box::new(std::io::Error::other("failed to delete session")))
        })?;
    Ok(ApiOk::ok(json!("Session closed successfully.")))
}

/// POST /auth/logout/all — revoke every session + clear the cookies.
pub async fn logout_all_devices(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<axum::response::Response, ApiError> {
    session_repo::delete_all_by_user(&state.db, auth.user_id())
        .await
        .map_err(|_| {
            ApiError::Internal(Box::new(std::io::Error::other("failed to delete sessions")))
        })?;

    let resp = (
        axum::http::StatusCode::OK,
        Json(json!({
            "status": "success",
            "timestamp": now_ms(),
            "result": "Logged out from all devices successfully.",
        })),
    )
        .into_response();
    let mut resp = resp;
    {
        let headers = resp.headers_mut();
        for c in [
            cookies::clear_access_cookie(),
            cookies::clear_refresh_cookie(),
        ] {
            if let Ok(v) = axum::http::HeaderValue::from_str(&c.to_string()) {
                headers.append(axum::http::header::SET_COOKIE, v);
            }
        }
    }
    Ok(resp)
}

#[derive(Debug, Deserialize)]
pub struct RequestResetBody {
    #[serde(default)]
    pub username: String,
}

fn reset_jwt(user_id: Uuid) -> Result<String, ApiError> {
    // Go signResetToken: RegisteredClaims with sub + 20-minute exp, HS256.
    let exp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64 + 20 * 60)
        .unwrap_or(0);
    let claims = serde_json::json!({
        "sub": user_id.to_string(),
        "exp": exp,
    });
    let secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "development-jwt-secret".into());
    jsonwebtoken::encode(
        &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| {
        tracing::error!(error = %e, "reset token sign failed");
        ApiError::Internal(Box::new(std::io::Error::other("failed to sign token")))
    })
}

/// POST /auth/request-reset — always answers "Job done." (user-existence
/// non-disclosure parity); silently skips unknown users / no-email accounts.
pub async fn request_reset(
    State(state): State<AppState>,
    Json(body): Json<RequestResetBody>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let Some(mail_client) = crate::mail::from_env() else {
        // Go: controller ignores the service error; the service returns
        // "mail client not configured" without sending. Same observable
        // behavior: 200 "Job done."
        return Ok(ApiOk::ok(json!("Job done.")));
    };

    if body.username.is_empty() {
        return Err(ApiError::BadRequest("username is required".into()));
    }

    let mut db = state.toasty.clone();
    let user = crate::models::User::filter_by_username(&body.username)
        .first()
        .exec(&mut db)
        .await
        .ok()
        .flatten();

    let Some(user) = user else {
        return Ok(ApiOk::ok(json!("Job done."))); // don't reveal existence
    };
    let Some(email) = user.email.clone() else {
        return Ok(ApiOk::ok(json!("Job done.")));
    };

    let reset_token = reset_jwt(user.id)?;
    sqlx::query("UPDATE users SET password_reset_token = $1 WHERE id = $2")
        .bind(&reset_token)
        .bind(user.id)
        .execute(&state.db)
        .await
        .map_err(ApiError::from)?;

    let frontend = std::env::var("FRONTEND_URL").unwrap_or_default();
    let reset_url = format!("{frontend}/login/reset?token={}", reset_token);
    let html = crate::mail::render_go_template(
        crate::mail::PASSWORD_RESET_TEMPLATE,
        &[("ResetURL", &reset_url), ("FrontendURL", &frontend)],
    );
    let plain = format!(
        "Your password reset link is: {}\n\nIf you did not expect this email, please ignore it.",
        reset_url
    );
    if let Err(e) = mail_client.send_html(&email, "Pergame: Reset your password", &plain, &html) {
        tracing::error!(error = %e, "failed to send password reset email");
    }

    Ok(ApiOk::ok(json!("Job done.")))
}

#[derive(Debug, Deserialize)]
pub struct ResetPasswordBody {
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub password: String,
}

/// POST /auth/reset-password — verify the reset JWT (HS256, 20 min), require
/// it to match the stored token, then set the new bcrypt hash.
pub async fn reset_password(
    State(state): State<AppState>,
    Json(body): Json<ResetPasswordBody>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    if body.token.is_empty() || body.password.is_empty() {
        return Err(ApiError::BadRequest("invalid reset token".into()));
    }

    let secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "development-jwt-secret".into());
    let validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
    let decoded = jsonwebtoken::decode::<serde_json::Value>(
        &body.token,
        &jsonwebtoken::DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|_| ApiError::BadRequest("invalid reset token".into()))?;

    let user_id: Uuid = decoded
        .claims
        .get("sub")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
        .ok_or(ApiError::BadRequest(
            "invalid user ID in reset token".into(),
        ))?;

    let stored: Option<Option<String>> =
        sqlx::query_scalar("SELECT password_reset_token FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_one(&state.db)
            .await
            .ok();
    let Some(Some(stored_token)) = stored else {
        return Err(ApiError::BadRequest("invalid reset token".into()));
    };
    if stored_token != body.token {
        return Err(ApiError::BadRequest("invalid reset token".into()));
    }

    let hash = bcrypt::hash(&body.password, bcrypt::DEFAULT_COST)
        .map_err(|_| ApiError::BadRequest("failed to hash password".into()))?;

    sqlx::query("UPDATE users SET password = $1, password_reset_token = NULL WHERE id = $2")
        .bind(&hash)
        .bind(user_id)
        .execute(&state.db)
        .await
        .map_err(ApiError::from)?;

    Ok(ApiOk::ok(json!("Password reset successfully.")))
}
