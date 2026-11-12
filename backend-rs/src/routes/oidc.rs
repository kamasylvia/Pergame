//! /auth/oidc routes — providers, authorize, callback, linked, link, unlink
//! (Go controllers/oidc.controller.go + services/oidc.service.go parity).

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use rand::RngExt;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::AppState;
use crate::auth::{AuthUser, cookies, jwt};
use crate::error::ApiError;
use crate::oidc::{SharedManager, UserInfo};
use crate::repo::oidc as oidc_repo;
use crate::response::ApiOk;

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn ok_json(result: serde_json::Value) -> Response {
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

fn envelope_with_cookies(
    status: StatusCode,
    body: serde_json::Value,
    set_cookies: Vec<cookie::Cookie<'static>>,
) -> Response {
    let resp = (
        status,
        Json(json!({
            "status": "success",
            "timestamp": now_ms(),
            "result": body,
        })),
    )
        .into_response();
    let mut resp = resp;
    {
        let headers = resp.headers_mut();
        for c in set_cookies {
            if let Ok(v) = axum::http::HeaderValue::from_str(&c.to_string()) {
                headers.append(header::SET_COOKIE, v);
            }
        }
    }
    resp
}

fn manager(state: &AppState) -> Result<SharedManager, ApiError> {
    state
        .oidc
        .clone()
        .ok_or(ApiError::NotFound("OIDC is not configured".into()))
}

/// GET /auth/oidc/providers
pub async fn get_providers(State(state): State<AppState>) -> Result<Response, ApiError> {
    let manager = manager(&state)?;
    if !manager.enabled() {
        return Err(ApiError::NotFound("OIDC is not configured".into()));
    }
    Ok(ok_json(json!({
        "enabled": true,
        "providers": manager.get_providers(),
    })))
}

#[derive(Debug, Deserialize)]
pub struct AuthorizeQuery {
    #[serde(default)]
    pub redirect_uri: String,
}

/// GET /auth/oidc/:provider/authorize?redirect_uri=
pub async fn authorize(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    Query(q): Query<AuthorizeQuery>,
) -> Result<Response, ApiError> {
    let manager = manager(&state)?;
    let provider = provider.to_lowercase();
    if provider.is_empty() {
        return Err(ApiError::BadRequest("provider name is required".into()));
    }
    if q.redirect_uri.is_empty() {
        return Err(ApiError::BadRequest("redirect_uri is required".into()));
    }

    let (auth_url, state_token) = manager
        .generate_auth_url(&provider, &q.redirect_uri, None)
        .await
        .map_err(ApiError::BadRequest)?;

    Ok(ok_json(json!({
        "auth_url": auth_url,
        "state": state_token,
        "redirect_uri": q.redirect_uri,
    })))
}

#[derive(Debug, Deserialize)]
pub struct CallbackBody {
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub redirect_uri: String,
}

/// POST /auth/oidc/:provider/callback — state validation (one-time), code
/// exchange, link flow or login-or-create with the double cookies.
pub async fn callback(
    State(state): State<AppState>,
    ctx: crate::extract::RequestCtx,
    Path(provider): Path<String>,
    Json(body): Json<CallbackBody>,
) -> Result<Response, ApiError> {
    let manager = manager(&state)?;
    let provider = provider.to_lowercase();

    let state_data = manager
        .validate_state(&body.state)
        .await
        .map_err(|_| ApiError::Unauthorized("invalid or expired state".into()))?;

    let p = manager
        .get_provider(&provider)
        .map_err(ApiError::BadRequest)?;
    let token = manager
        .exchange_code(&p, &body.code, &body.redirect_uri)
        .await
        .map_err(|e| ApiError::Unauthorized(format!("failed to exchange code: {e}")))?;
    let user_info: UserInfo = manager
        .get_user_info(&p, &token.access_token)
        .await
        .map_err(|e| ApiError::Unauthorized(format!("failed to get user info: {e}")))?;

    // Link flow: logged-in user redeeming a link state.
    if state_data.is_link {
        oidc_repo::link_provider(&state.db, state_data.user_id, &provider, &user_info.sub).await?;
        return Ok(ok_json(json!({
            "linked": true,
            "provider": provider,
        })));
    }

    // Login-or-create.
    let mut db = state.toasty.clone();
    let user = oidc_login_or_create(&mut db, &state, &manager, &provider, &user_info, &ctx).await?;
    let _is_new = false; // parity detail: LoginOrCreate signals new; surfaced via created timestamps

    let cfg = crate::config::global();
    let token = jwt::sign_access_token(user.id, user.role, cfg.auth.access_token_expiry)
        .map_err(|_| ApiError::Internal(Box::new(std::io::Error::other("failed to sign token"))))?;
    let refresh_token = crate::auth::totp::create_login_session_type(
        &mut db,
        user.id,
        &ctx.ip,
        &ctx.user_agent,
        "login_oidc",
    )
    .await?;

    Ok(envelope_with_cookies(
        StatusCode::OK,
        json!({
            "user": serde_json::to_value(&user).unwrap_or(json!(null)),
            "is_new": user.created_timestamp == now_ms(),
            "provider": provider,
        }),
        vec![
            cookies::access_cookie(&token, cfg.auth.access_token_expiry),
            cookies::refresh_cookie(&refresh_token, cfg.auth.refresh_token_expiry),
        ],
    ))
}

#[derive(Debug, Deserialize)]
pub struct LinkQuery {
    #[serde(default)]
    pub redirect_uri: String,
}

/// POST /auth/oidc/:provider/link?redirect_uri= — link flow for the logged-in
/// user (state carries the user id).
pub async fn link_provider(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(provider): Path<String>,
    Query(q): Query<LinkQuery>,
) -> Result<Response, ApiError> {
    let manager = manager(&state)?;
    let provider = provider.to_lowercase();
    if provider.is_empty() {
        return Err(ApiError::BadRequest("provider name is required".into()));
    }
    if q.redirect_uri.is_empty() {
        return Err(ApiError::BadRequest("redirect_uri is required".into()));
    }
    let (auth_url, state_token) = manager
        .generate_auth_url(&provider, &q.redirect_uri, Some(auth.user_id()))
        .await
        .map_err(ApiError::BadRequest)?;

    Ok(ok_json(json!({
        "auth_url": auth_url,
        "state": state_token,
        "redirect_uri": q.redirect_uri,
    })))
}

/// DELETE /auth/oidc/:provider/unlink
pub async fn unlink_provider(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(provider): Path<String>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let provider = provider.to_lowercase();
    if provider.is_empty() {
        return Err(ApiError::BadRequest("provider name is required".into()));
    }

    let links = oidc_repo::get_by_user(&state.db, auth.user_id())
        .await
        .map_err(ApiError::from)?;
    let has_password = oidc_repo::has_password(&state.db, auth.user_id())
        .await
        .map_err(|_| ApiError::BadRequest("failed to get user password status".into()))?;
    if !has_password && links.len() <= 1 {
        return Err(ApiError::BadRequest(
            "cannot unlink the only login method".into(),
        ));
    }

    for link in links {
        if link.provider_name.to_lowercase() == provider {
            oidc_repo::delete(&state.db, link.id)
                .await
                .map_err(ApiError::from)?;
            return Ok(ApiOk::ok(json!("Provider unlinked successfully")));
        }
    }
    Err(ApiError::BadRequest("provider link not found".into()))
}

/// GET /auth/oidc/linked — the caller's linked providers.
pub async fn get_linked(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<ApiOk<Vec<serde_json::Value>>, ApiError> {
    let links = oidc_repo::get_by_user(&state.db, auth.user_id())
        .await
        .map_err(ApiError::from)?;
    let value: Vec<serde_json::Value> = links
        .iter()
        .map(|l| serde_json::to_value(l).unwrap_or(json!(null)))
        .collect();
    Ok(ApiOk::ok(value))
}

/// Go LoginOrCreate: existing link → login; else create user + link.
async fn oidc_login_or_create(
    db: &mut toasty::Db,
    state: &AppState,
    _manager: &SharedManager,
    provider: &str,
    user_info: &UserInfo,
    ctx: &crate::extract::RequestCtx,
) -> Result<crate::models::User, ApiError> {
    let existing = oidc_repo::get_by_provider_and_subject(&state.db, provider, &user_info.sub)
        .await
        .map_err(|e| ApiError::Internal(Box::new(e)))?;

    let (user_id, is_new) = match &existing {
        Some(link) => (link.user_id, false),
        None => (create_user_from_oidc(db, state, user_info).await?, true),
    };

    let user = crate::models::User::filter_by_id(user_id)
        .first()
        .exec(db)
        .await
        .ok()
        .flatten()
        .ok_or(ApiError::Unauthorized("linked user not found".into()))?;

    if is_new {
        oidc_repo::link_provider(&state.db, user_id, provider, &user_info.sub).await?;
    }

    crate::auth::totp::create_login_session_type(
        db,
        user_id,
        &ctx.ip,
        &ctx.user_agent,
        "login_oidc",
    )
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "oidc session create failed");
        ApiError::Unauthorized("failed to create session".into())
    })?;

    Ok(user)
}

/// Go createUserFromOIDC: unique username generation, optional names, http(s)
/// avatar (≤2048 bytes), role 1.
async fn create_user_from_oidc(
    db: &mut toasty::Db,
    _state: &AppState,
    user_info: &UserInfo,
) -> Result<Uuid, ApiError> {
    let user_id = crate::types::new_id();
    let username = generate_unique_username(db, user_info.preferred_username.as_deref(), user_id);
    let now = now_ms();

    let avatar = user_info.picture.as_deref().and_then(|p| {
        if p.len() <= 2048 && (p.starts_with("https://") || p.starts_with("http://")) {
            Some(p.to_string())
        } else {
            None
        }
    });

    crate::models::User::create()
        .id(user_id)
        .username(username)
        .firstname(user_info.given_name.clone().filter(|s| !s.is_empty()))
        .lastname(user_info.family_name.clone().filter(|s| !s.is_empty()))
        .role(1)
        .user_type(0)
        .avatar(avatar)
        .email(user_info.email.clone())
        .totp_enabled(false)
        .totp_forced(false)
        .created_timestamp(now)
        .updated_timestamp(now)
        .exec(db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "oidc user create failed");
            ApiError::Unauthorized("failed to create user".into())
        })?;
    Ok(user_id)
}

/// Go GenerateUniqueUsername: sanitized preferred name (or "Alex-{adjective}")
/// + "-%06d" of the id mod 1e6.
fn generate_unique_username(_db: &mut toasty::Db, given: Option<&str>, user_id: Uuid) -> String {
    let mut base = String::new();
    if let Some(name) = given {
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            base = trimmed.replace(' ', "_");
            base = base
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                .collect();
            if base.len() > 20 {
                base = base.chars().take(20).collect();
            }
        }
    }
    if base.is_empty() {
        const ADJECTIVES: [&str; 15] = [
            "Brave", "Clever", "Swift", "Mighty", "Nimble", "Wise", "Bold", "Fierce", "Loyal",
            "Gentle", "Bright", "Calm", "Daring", "Epic", "Fast",
        ];
        let adj = ADJECTIVES[rand::rng().random_range(0..ADJECTIVES.len())];
        base = format!("Alex-{adj}");
    }
    let suffix = format!("{:06}", user_id.as_u128() % 1_000_000);
    format!("{base}-{suffix}")
}
