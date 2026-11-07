//! /auth/2fa routes (Go controllers/authentication.controller.go parity):
//! request (enroll), confirm (activate + backup codes), verify (complete
//! pre-auth login), disable.

use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::auth::service::now_ms;
use crate::auth::{cookies, jwt, totp, AuthUser};
use crate::error::ApiError;
use crate::AppState;

fn ok_envelope(result: serde_json::Value) -> Response {
    (
        StatusCode::OK,
        Json(json!({
            "status": "success",
            "timestamp": now_ms(),
            "result": result,
        })),
    )
        .into_response()
}

fn envelope_with_cookies(status: StatusCode, body: serde_json::Value, set_cookies: Vec<cookie::Cookie<'static>>) -> Response {
    let resp = (status, Json(json!({
        "status": "success",
        "timestamp": now_ms(),
        "result": body,
    }))).into_response();
    let mut resp = resp;
    {
        let headers = resp.headers_mut();
        for c in set_cookies {
            if let Ok(v) = header::HeaderValue::from_str(&c.to_string()) {
                headers.append(header::SET_COOKIE, v);
            }
        }
    }
    resp
}

/// Resolves the acting user id: session cookie when logged in, otherwise the
/// `pre_auth_token` body field (forced-enrollment login flow).
async fn resolve_user_id(
    state: &AppState,
    auth: Option<AuthUser>,
    pre_auth_token: Option<String>,
) -> Result<i64, ApiError> {
    if let Some(auth) = auth {
        return Ok(auth.user_id());
    }
    let token = pre_auth_token
        .filter(|t| !t.is_empty())
        .ok_or(ApiError::BadRequest("pre_auth_token is required".into()))?;
    jwt::verify_pre_auth_token(&token)
        .map(|id| id as i64)
        .map_err(|_| ApiError::Unauthorized("invalid pre-authentication token".into()))
}

#[derive(Debug, Deserialize, Default)]
pub struct PreAuthBody {
    #[serde(default)]
    pub pre_auth_token: String,
}

/// POST /auth/2fa/request — generate an enrollment secret + otpauth URL.
pub async fn request_activation(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
    body: Option<Json<PreAuthBody>>,
) -> Result<Response, ApiError> {
    let pre_token = body.map(|b| b.pre_auth_token.clone());
    let user_id = resolve_user_id(&state, auth, pre_token).await?;

    let mut db = state.toasty.clone();
    let user = crate::models::User::filter_by_id(user_id)
        .first()
        .exec(&mut db)
        .await
        .ok()
        .flatten()
        .ok_or(ApiError::BadRequest("user not found".into()))?;

    let (secret, qr_code_url) = totp::generate_secret(&user.username)?;
    Ok(ok_envelope(json!({
        "secret": secret,
        "qr_code_url": qr_code_url,
    })))
}

#[derive(Debug, Deserialize)]
pub struct ConfirmBody {
    #[serde(default)]
    pub pre_auth_token: String,
    #[serde(default)]
    pub secret: String,
    #[serde(default)]
    pub code: String,
}

/// POST /auth/2fa/confirm — validate the enrollment code, store backup codes,
/// enable TOTP. Returns the plain backup codes (shown once).
pub async fn confirm_activation(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
    Json(body): Json<ConfirmBody>,
) -> Result<Response, ApiError> {
    if body.secret.is_empty() || body.code.is_empty() {
        return Err(ApiError::BadRequest("secret and code are required".into()));
    }
    let pre_token = if auth.is_some() { None } else { Some(body.pre_auth_token.clone()) };
    let user_id = resolve_user_id(&state, auth, pre_token).await?;

    let mut db = state.toasty.clone();
    let plain_codes = totp::enable_totp(&mut db, &state.snowflake, user_id, &body.secret, &body.code).await?;
    Ok(ok_envelope(json!({ "backup_codes": plain_codes })))
}

#[derive(Debug, Deserialize)]
pub struct VerifyBody {
    #[serde(default)]
    pub pre_auth_token: String,
    #[serde(default)]
    pub code: String,
}

/// POST /auth/2fa/verify — completes a TOTP login started with pre-auth.
pub async fn verify_totp(
    State(state): State<AppState>,
    ctx: crate::extract::RequestCtx,
    Json(body): Json<VerifyBody>,
) -> Result<Response, ApiError> {
    if body.pre_auth_token.is_empty() || body.code.is_empty() {
        return Err(ApiError::BadRequest("pre_auth_token and code are required".into()));
    }

    let mut db = state.toasty.clone();
    let user = totp::verify_pre_auth_and_user(&mut db, &body.pre_auth_token, &body.code).await?;

    let refresh_token =
        totp::create_login_session(&mut db, &state.snowflake, user.id, &ctx.ip, &ctx.user_agent).await?;

    let cfg = crate::config::global();
    let token = jwt::sign_access_token(user.id as u64, user.role, cfg.auth.access_token_expiry)
        .map_err(|_| ApiError::Internal(Box::new(std::io::Error::other("failed to sign token"))))?;

    Ok(envelope_with_cookies(
        StatusCode::OK,
        serde_json::to_value(&user).unwrap_or(json!(null)),
        vec![
            cookies::access_cookie(&token, cfg.auth.access_token_expiry),
            cookies::refresh_cookie(&refresh_token, cfg.auth.refresh_token_expiry),
        ],
    ))
}

#[derive(Debug, Deserialize)]
pub struct DisableBody {
    #[serde(default)]
    pub code: String,
}

/// POST /auth/2fa/disable — session-authenticated; validates the code, clears
/// the secret, deletes backup codes.
pub async fn disable_totp(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<DisableBody>,
) -> Result<Response, ApiError> {
    if body.code.is_empty() {
        return Err(ApiError::BadRequest("code is required".into()));
    }
    let mut db = state.toasty.clone();
    totp::disable_totp(&mut db, auth.user_id(), &body.code).await?;
    Ok(ok_envelope(json!("2FA disabled successfully.")))
}
