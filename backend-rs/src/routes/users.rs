//! /users routes — signup (public), @me and get-by-id (authed).

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Deserialize;

use crate::AppState;
use crate::auth::AuthUser;
use crate::auth::service::{self, SignupInput};
use crate::error::ApiError;
use crate::response::ApiOk;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
/// POST /users body (public signup).
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
pub async fn create_user(
    State(state): State<AppState>,
    Json(body): Json<CreateUserBody>,
) -> Result<ApiOk<crate::models::User>, ApiError> {
    if matches!(
        std::env::var("CONFIG_DISABLE_SIGNUP").as_deref(),
        Ok("true")
    ) {
        return Err(ApiError::Forbidden("user signup is disabled".into()));
    }

    // Go binding: username required 5..=30; password omitempty 4..=50; email format.
    if body.username.is_empty() {
        return Err(ApiError::BadRequest("Username is required.".into()));
    }
    if body.username.len() < 5 || body.username.len() > 30 {
        return Err(ApiError::BadRequest(
            "Username must be between 5 and 30 characters.".into(),
        ));
    }
    if let Some(p) = body.password.as_deref()
        && !p.is_empty()
        && (p.len() < 4 || p.len() > 50)
    {
        return Err(ApiError::BadRequest(
            "Password must be between 4 and 50 characters.".into(),
        ));
    }
    if let Some(e) = body.email.as_deref()
        && !e.is_empty()
        && !e.contains('@')
    {
        return Err(ApiError::BadRequest("Invalid email address.".into()));
    }

    let mut db = state.toasty.clone();
    let user = service::signup(
        &mut db,
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
pub async fn get_me(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<ApiOk<crate::models::User>, ApiError> {
    let mut db = state.toasty.clone();
    let user = fetch_user(&mut db, auth.user_id()).await?;
    Ok(ApiOk::ok(user))
}

/// GET /users/@me/quota — resolved upload limits + current usage for the
/// CDN storage bar. `max_* = null` means unlimited.
pub async fn get_my_quota(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let quota = crate::quota::for_user(&state.db, auth.user_id(), auth.role).await;
    let used = crate::repo::nodes::uploads_size(&state.db, auth.user_id())
        .await
        .map_err(ApiError::from)?;
    Ok(ApiOk::ok(serde_json::json!({
        "used_bytes": used,
        "max_file_size": quota.max_file,
        "max_total_size": quota.max_total,
    })))
}

/// GET /users/:id
pub async fn get_user(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<String>,
) -> Result<ApiOk<crate::models::User>, ApiError> {
    let uid = Uuid::parse_str(&id).map_err(|_| ApiError::BadRequest("Invalid user ID".into()))?;
    let mut db = state.toasty.clone();
    let user = fetch_user(&mut db, uid).await?;
    Ok(ApiOk::ok(user))
}

async fn fetch_user(db: &mut toasty::Db, id: Uuid) -> Result<crate::models::User, ApiError> {
    crate::models::User::filter_by_id(id)
        .first()
        .exec(db)
        .await
        .ok()
        .flatten()
        .ok_or(ApiError::NotFound("user not found".into()))
}

/// DELETE /users/:id — delete an account (self or admin).
///
/// Snapshot gate: refused while the user has snapshots on any reachable
/// target — deleting the account must not orphan backups the user paid
/// storage for. Empty the targets first (or delete them); unreachable
/// targets also refuse, since emptiness cannot be proven (fork §8
/// deleteCheck parity).
///
/// What goes away: the users row (FK cascades clear nodes, sessions,
/// settings, links, permissions, invitations) plus the `{uid}/` attachment
/// prefix in the service bucket. Target objects are the user's own S3 and
/// are never touched here.
pub async fn delete_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let uid = Uuid::parse_str(&id).map_err(|_| ApiError::BadRequest("Invalid user ID".into()))?;
    if auth.user_id() != uid && !crate::auth::guard::is_app_admin(auth.role) {
        return Err(ApiError::Forbidden("forbidden".into()));
    }
    // The last admin cannot delete themselves (lockout prevention).
    if auth.user_id() == uid && crate::auth::guard::is_app_admin(auth.role) {
        let (admins,): (i64,) = sqlx::query_as("SELECT count(*) FROM users WHERE role = 2")
            .fetch_one(&state.db)
            .await
            .unwrap_or((1,));
        if admins <= 1 {
            return Err(ApiError::BadRequest(
                "cannot delete the last admin account".into(),
            ));
        }
    }
    // Snapshot gate across all of the user's targets.
    let targets = crate::target::repo::list_targets(&state.db).await?;
    for t in targets.into_iter().filter(|t| t.users.contains(&uid)) {
        let conn = t.connect().await.map_err(|_| {
            ApiError::Conflict(format!(
                "target '{}' is unreachable; prove it is empty before deleting this account",
                t.name
            ))
        })?;
        let prefix = format!("{uid}/manifests/");
        let has_snapshots = conn
            .list(&prefix)
            .await
            .map(|objs| objs.iter().any(|(k, _, _)| k.contains("/manifests/")))
            .unwrap_or(true); // list failure = cannot prove empty
        if has_snapshots {
            return Err(ApiError::Conflict(format!(
                "account has snapshots on target '{}'; empty the target first",
                t.name
            )));
        }
    }
    // Attachments first (best-effort), then the row (cascades the rest).
    // Blob references release one by one (shared bytes survive until the
    // last user is gone); the legacy per-user prefix is swept as before.
    if let Some(storage) = &state.storage {
        let shas: Vec<(String,)> = sqlx::query_as(
            "SELECT metadata->>'content_sha256' FROM nodes WHERE user_id = $1 AND role = 4
             AND metadata->>'content_sha256' IS NOT NULL",
        )
        .bind(uid)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();
        for (sha,) in shas {
            crate::blobs::release(&state.db, storage, &sha).await;
        }
        storage.delete_user_prefix(uid).await;
    }
    let mut db = state.toasty.clone();
    crate::models::User::filter_by_id(uid)
        .delete()
        .exec(&mut db)
        .await
        .map_err(|e| ApiError::BadRequest(format!("account deletion failed: {e}")))?;
    Ok(ApiOk::ok(serde_json::json!("Account deleted.")))
}
