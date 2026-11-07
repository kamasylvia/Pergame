//! /users routes — signup (public), @me and get-by-id (authed).

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

use crate::auth::service::{self, SignupInput};
use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::response::ApiOk;
use crate::types::Snowflake;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct CreateUserBody {
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub firstname: Option<String>,
    #[serde(default)]
    pub lastname: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub user_type: Option<i32>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub totp_forced: bool,
}

/// POST /users — signup (Go CreateUser parity: binding validation, duplicate
/// check, bcrypt cost 10, role always 1; empty password goes through the
/// mail-setup flow which fails without a mail client).
pub async fn create_user(State(state): State<AppState>, Json(body): Json<CreateUserBody>) -> Result<ApiOk<crate::models::User>, ApiError> {
    if matches!(std::env::var("CONFIG_DISABLE_SIGNUP").as_deref(), Ok("true")) {
        return Err(ApiError::Forbidden("user signup is disabled".into()));
    }

    // Go binding: username required 5..=30; password omitempty 4..=50; email format.
    if body.username.is_empty() {
        return Err(ApiError::BadRequest("Username is required.".into()));
    }
    if body.username.len() < 5 || body.username.len() > 30 {
        return Err(ApiError::BadRequest("Username must be between 5 and 30 characters.".into()));
    }
    if let Some(p) = body.password.as_deref()
        && !p.is_empty() && (p.len() < 4 || p.len() > 50) {
            return Err(ApiError::BadRequest("Password must be between 4 and 50 characters.".into()));
        }
    if let Some(e) = body.email.as_deref()
        && !e.is_empty() && !e.contains('@') {
            return Err(ApiError::BadRequest("Invalid email address.".into()));
        }

    let mut db = state.toasty.clone();
    let user = service::signup(
        &mut db,
        &state.snowflake,
        SignupInput {
            username: body.username,
            firstname: body.firstname.filter(|s| !s.is_empty()),
            lastname: body.lastname.filter(|s| !s.is_empty()),
            avatar: body.avatar.filter(|s| !s.is_empty()),
            email: body.email.filter(|s| !s.is_empty()),
            user_type: body.user_type.unwrap_or(0),
            password: body.password.filter(|s| !s.is_empty()),
            totp_forced: body.totp_forced,
        },
    )
    .await?;

    Ok(ApiOk::with_status(StatusCode::CREATED, user))
}

/// GET /users/@me
pub async fn get_me(State(state): State<AppState>, auth: AuthUser) -> Result<ApiOk<crate::models::User>, ApiError> {
    let mut db = state.toasty.clone();
    let user = fetch_user(&mut db, auth.user_id()).await?;
    Ok(ApiOk::ok(user))
}

/// GET /users/:id
pub async fn get_user(State(state): State<AppState>, _auth: AuthUser, Path(id): Path<String>) -> Result<ApiOk<crate::models::User>, ApiError> {
    let snowflake = Snowflake::parse(&id).ok_or_else(|| ApiError::BadRequest("Invalid user ID".into()))?;
    let mut db = state.toasty.clone();
    let user = fetch_user(&mut db, snowflake.as_i64()).await?;
    Ok(ApiOk::ok(user))
}

async fn fetch_user(db: &mut toasty::Db, id: i64) -> Result<crate::models::User, ApiError> {
    crate::models::User::filter_by_id(id)
        .first()
        .exec(db)
        .await
        .ok()
        .flatten()
        .ok_or(ApiError::NotFound("user not found".into()))
}
