//! /auth routes — login (form or JSON), refresh, logout (Go controller parity).

use axum::Json;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;

use crate::AppState;
use crate::auth::{
    cookies, jwt,
    service::{self, LoginOutcome},
};
use crate::error::ApiError;

fn envelope_with_cookies(
    status: StatusCode,
    body: serde_json::Value,
    cookies: Vec<cookie::Cookie<'static>>,
) -> Response {
    let envelope = json!({
        "status": "success",
        "timestamp": service::now_ms(),
        "result": body,
    });
    let resp = (status, Json(envelope)).into_response();
    let mut resp = resp;
    {
        let headers = resp.headers_mut();
        for cookie in cookies {
            if let Ok(v) = header::HeaderValue::from_str(&cookie.to_string()) {
                headers.append(header::SET_COOKIE, v);
            }
        }
    }
    resp
}

/// Reads a cookie value from the request's Cookie header (manual parse).
fn get_cookie(req_headers: &axum::http::HeaderMap, name: &str) -> Option<String> {
    req_headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|c| {
                let c = c.trim();
                c.strip_prefix(&format!("{name}="))
                    .map(|v| v.trim().to_string())
            })
        })
}

#[derive(Debug, Deserialize)]
pub struct LoginInput {
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
}

/// Accepts `application/x-www-form-urlencoded` (Go tests) and JSON alike.
pub async fn parse_login_body(req: axum::extract::Request) -> Result<LoginInput, ApiError> {
    let content_type = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();

    let body = axum::body::to_bytes(req.into_body(), 64 * 1024)
        .await
        .map_err(|_| ApiError::BadRequest("failed to read request body".into()))?;

    if content_type.contains("application/json") {
        serde_json::from_slice::<LoginInput>(&body)
            .map_err(|_| ApiError::BadRequest("username and password are required".into()))
    } else {
        // form-urlencoded
        let mut username = String::new();
        let mut password = String::new();
        for (k, v) in form_urlencoded::parse(&body) {
            match k.as_ref() {
                "username" => username = v.into_owned(),
                "password" => password = v.into_owned(),
                _ => {}
            }
        }
        if username.is_empty() {
            return Err(ApiError::BadRequest("Username is required.".into()));
        }
        if password.is_empty() {
            return Err(ApiError::BadRequest("Password is required.".into()));
        }
        Ok(LoginInput { username, password })
    }
}

pub async fn login(
    State(state): State<AppState>,
    ctx: crate::extract::RequestCtx,
    req: axum::extract::Request,
) -> Result<Response, ApiError> {
    if matches!(
        std::env::var("CONFIG_DISABLE_NATIVE_LOGIN").as_deref(),
        Ok("true")
    ) {
        return Err(ApiError::Forbidden("native login is disabled".into()));
    }

    let input = parse_login_body(req).await?;
    let mut db = state.toasty.clone();
    let outcome = service::login(
        &mut db,
        &input.username,
        &input.password,
        &ctx.ip,
        &ctx.user_agent,
    )
    .await?;

    match outcome {
        LoginOutcome::TwoFa {
            message,
            pre_auth_token,
            user_id,
        } => Ok(envelope_with_cookies(
            StatusCode::ACCEPTED,
            json!({
                "message": message,
                "pre_auth_token": pre_auth_token,
                "user_id": user_id,
            }),
            vec![],
        )),
        LoginOutcome::Full {
            user,
            refresh_token,
        } => {
            let cfg = crate::config::global();
            let token = jwt::sign_access_token(user.id, user.role, cfg.auth.access_token_expiry)
                .map_err(|_| {
                    ApiError::Internal(Box::new(std::io::Error::other("failed to sign token")))
                })?;
            Ok(envelope_with_cookies(
                StatusCode::OK,
                serde_json::to_value(&user).unwrap_or(json!(null)),
                vec![
                    cookies::access_cookie(&token, cfg.auth.access_token_expiry),
                    cookies::refresh_cookie(&refresh_token, cfg.auth.refresh_token_expiry),
                ],
            ))
        }
    }
}

pub async fn refresh(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<Response, ApiError> {
    let Some(refresh_token) = get_cookie(&headers, cookies::REFRESH_COOKIE) else {
        return Err(ApiError::Unauthorized("no refresh token provided".into()));
    };

    let mut db = state.toasty.clone();
    let (user, new_refresh) = service::refresh_session(&mut db, &refresh_token).await?;

    let cfg = crate::config::global();
    let token = jwt::sign_access_token(user.id, user.role, cfg.auth.access_token_expiry)
        .map_err(|_| ApiError::Internal(Box::new(std::io::Error::other("failed to sign token"))))?;

    Ok(envelope_with_cookies(
        StatusCode::OK,
        json!("Session refreshed successfully."),
        vec![
            cookies::access_cookie(&token, cfg.auth.access_token_expiry),
            cookies::refresh_cookie(&new_refresh, cfg.auth.refresh_token_expiry),
        ],
    ))
}

pub async fn logout(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<Response, ApiError> {
    let mut db = state.toasty.clone();
    if let Some(token) = get_cookie(&headers, cookies::REFRESH_COOKIE) {
        service::logout(&mut db, &token).await;
    }
    Ok(envelope_with_cookies(
        StatusCode::OK,
        json!("Logged out successfully."),
        vec![
            cookies::clear_access_cookie(),
            cookies::clear_refresh_cookie(),
        ],
    ))
}
