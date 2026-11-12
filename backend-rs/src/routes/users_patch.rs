//! PATCH /users/{id} and PATCH /users/{id}/password (Go UpdateUser /
//! UpdatePassword parity: self-or-admin access, admin-only fields, password
//! change rules).

use axum::Json;
use axum::extract::{Path, State};
use serde::Deserialize;
use serde_json::json;

use crate::AppState;
use crate::auth::AuthUser;
use crate::auth::service::now_ms;
use crate::error::ApiError;
use crate::models::User;
use crate::response::ApiOk;
use uuid::Uuid;

/// Go `utils.GetTargetId`: "@me" resolves to the actor, else UUID parse.
pub fn resolve_target(param: &str, auth: &AuthUser) -> Result<Uuid, ApiError> {
    if param.is_empty() {
        return Err(ApiError::BadRequest("parameter is empty".into()));
    }
    if param == "@me" {
        return Ok(auth.id);
    }
    Uuid::parse_str(param).map_err(|_| ApiError::BadRequest("invalid parameter".into()))
}

/// Go `RequireUserAccess`: the actor may act on themselves or be an app admin.
pub fn require_user_access(auth: &AuthUser, target: Uuid) -> Result<(), ApiError> {
    if auth.id == target || auth.role == 2 {
        Ok(())
    } else {
        Err(ApiError::Forbidden("forbidden".into()))
    }
}

pub const ROLE_ADMIN: i32 = 2;

#[derive(Debug, Deserialize)]
pub struct UpdateUserBody {
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
    pub totp_forced: Option<bool>,
}

pub async fn update_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(param): Path<String>,
    Json(body): Json<UpdateUserBody>,
) -> Result<ApiOk<User>, ApiError> {
    let target = resolve_target(&param, &auth)?;
    require_user_access(&auth, target)?;

    let mut db = state.toasty.clone();
    let db_user = User::filter_by_id(target)
        .first()
        .exec(&mut db)
        .await
        .ok()
        .flatten()
        .ok_or(ApiError::NotFound("not found".into()))?;

    // Only admins may change type / TOTP-forced; type must be 0 or 1.
    let (user_type, totp_forced) = if auth.role == ROLE_ADMIN {
        if let Some(t) = body.user_type
            && t != 0
            && t != 1
        {
            return Err(ApiError::BadRequest("invalid user type".into()));
        }
        (
            body.user_type.unwrap_or(db_user.user_type),
            body.totp_forced.unwrap_or(db_user.totp_forced),
        )
    } else {
        (db_user.user_type, db_user.totp_forced)
    };

    User::filter_by_id(db_user.id)
        .update()
        .firstname(body.firstname.or(db_user.firstname.clone()))
        .lastname(body.lastname.or(db_user.lastname.clone()))
        .avatar(body.avatar.or(db_user.avatar.clone()))
        .email(body.email.or(db_user.email.clone()))
        .user_type(user_type)
        .totp_forced(totp_forced)
        .updated_timestamp(now_ms())
        .exec(&mut db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "user update failed");
            ApiError::BadRequest("failed to update user".into())
        })?;

    let updated = User::filter_by_id(db_user.id)
        .first()
        .exec(&mut db)
        .await
        .ok()
        .flatten()
        .ok_or(ApiError::NotFound("not found".into()))?;
    Ok(ApiOk::ok(updated))
}

#[derive(Debug, Deserialize)]
pub struct UpdatePasswordBody {
    #[serde(default)]
    pub current_password: String,
    #[serde(default)]
    pub password: String,
}

pub async fn update_password(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(param): Path<String>,
    Json(body): Json<UpdatePasswordBody>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let target = resolve_target(&param, &auth)?;
    require_user_access(&auth, target)?;

    if body.password.is_empty() {
        return Err(ApiError::BadRequest("password is required".into()));
    }

    let mut db = state.toasty.clone();
    let db_user = User::filter_by_id(target)
        .first()
        .exec(&mut db)
        .await
        .ok()
        .flatten()
        .ok_or(ApiError::NotFound("not found".into()))?;

    // Current-password confirmation applies only when changing one's own
    // password; admins resetting another user's password are exempt (account
    // recovery). First-time setters (OAuth-only accounts) skip the check too.
    if auth.id == target
        && let Some(hash) = &db_user.password
    {
        if body.current_password.is_empty() {
            return Err(ApiError::BadRequest("current password is required".into()));
        }
        if !bcrypt::verify(&body.current_password, hash).unwrap_or(false) {
            return Err(ApiError::Unauthorized(
                "current password is incorrect".into(),
            ));
        }
    }

    let new_hash = bcrypt::hash(&body.password, bcrypt::DEFAULT_COST)
        .map_err(|_| ApiError::BadRequest("failed to hash password".into()))?;

    User::filter_by_id(db_user.id)
        .update()
        .password(Some(new_hash))
        .updated_timestamp(now_ms())
        .exec(&mut db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "password update failed");
            ApiError::BadRequest("failed to update password".into())
        })?;

    Ok(ApiOk::ok(json!("Password updated successfully")))
}
