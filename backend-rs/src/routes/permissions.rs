//! Node permission + invitation routes (Go nodes.permissions.controller.go
//! + services/permission.service.go parity).

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::Deserialize;
use serde_json::json;

use crate::AppState;
use crate::auth::AuthUser;
use crate::auth::guard::{self, Actor, NodeAction};
use crate::error::ApiError;
use crate::repo::permissions as perm_repo;
use crate::response::ApiOk;

/// Toasty repo functions return plain error strings; map them to the envelope.
fn toasty_err(e: String) -> ApiError {
    ApiError::Internal(Box::new(std::io::Error::other(e)))
}
use uuid::Uuid;

pub const PERM_READ: i32 = 1;
pub const PERM_ADMIN: i32 = 3;

fn resolve(param: &str) -> Result<Uuid, ApiError> {
    if param.is_empty() {
        return Err(ApiError::BadRequest("parameter is empty".into()));
    }
    Uuid::parse_str(param).ok_or_else(|| ApiError::BadRequest("invalid parameter".into()))
}

/// Invitation code alphabet excludes confusable glyphs (Go parity).
const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

fn generate_invitation_code(length: usize) -> String {
    use rand::distr::{Distribution, Uniform};
    let mut rng = rand::rng();
    let dist = Uniform::new(0usize, CODE_ALPHABET.len()).expect("uniform range");
    (0..length)
        .map(|_| CODE_ALPHABET[dist.sample(&mut rng)] as char)
        .collect()
}

/// Go parseInvitationCode: bare code, `?code=` URL, or last URL path segment;
/// uppercase, exactly 6 chars, restricted alphabet.
fn parse_invitation_code(input: &str) -> Result<String, ApiError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(ApiError::NotFound("not found".into()));
    }

    let mut candidate = trimmed.to_string();
    if let Ok(url) = url::Url::parse(trimmed) {
        if let Some(code) = url
            .query_pairs()
            .find(|(k, _)| k == "code")
            .map(|(_, v)| v.to_string())
        {
            candidate = code;
        } else if let Some(last) = url.path().split('/').rfind(|s| !s.is_empty()) {
            candidate = last.to_string();
        }
    }

    let candidate = candidate.trim().to_uppercase();
    if candidate.len() != 6 {
        return Err(ApiError::BadRequest(
            "invalid invitation code length".into(),
        ));
    }
    if !candidate
        .chars()
        .all(|c| CODE_ALPHABET.contains(&(c as u8)))
    {
        return Err(ApiError::BadRequest(
            "invalid invitation code format".into(),
        ));
    }
    Ok(candidate)
}

fn invitation_json(row: &(i64, String, i32, i64, i64)) -> serde_json::Value {
    json!({
        "id": row.0.to_string(),
        "invitation_code": row.1,
        "permission_level": row.2,
        "node_id": row.3.to_string(),
        "created_timestamp": row.4,
    })
}

#[derive(Debug, Deserialize)]
pub struct PermissionsQuery {
    #[serde(default)]
    pub recursive: String,
}

/// GET /nodes/:nodeId/permissions?recursive=
pub async fn get_node_permissions(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(node_id): Path<String>,
    Query(q): Query<PermissionsQuery>,
) -> Result<ApiOk<Vec<crate::models::PermissionRow>>, ApiError> {
    let actor: Actor = auth.into();
    let id = resolve(&node_id)?;
    guard::require_node_action(&state.db, &actor, id, NodeAction::ManagePermissions).await?;

    let perms = if q.recursive == "true" {
        perm_repo::get_by_node_recursive(&state.db, id).await?
    } else {
        let mut tdb = state.toasty.clone();
        perm_repo::get_by_node(&mut tdb, id)
            .await
            .map_err(toasty_err)?
    };
    Ok(ApiOk::ok(perms))
}

#[derive(Debug, Deserialize)]
pub struct CreatePermissionBody {
    #[serde(default)]
    pub user_id: String,
    #[serde(default)]
    pub permission: i32,
}

/// POST /nodes/:nodeId/permissions
pub async fn create_permission(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(node_id): Path<String>,
    Json(body): Json<CreatePermissionBody>,
) -> Result<ApiOk<crate::models::PermissionRow>, ApiError> {
    let actor: Actor = auth.into();
    let node = resolve(&node_id)?;
    if !(PERM_READ..=PERM_ADMIN).contains(&body.permission) {
        return Err(ApiError::BadRequest("invalid permission level".into()));
    }
    guard::require_node_action(&state.db, &actor, node, NodeAction::ManagePermissions).await?;

    let user = Uuid::parse_str(&body.user_id)
        .ok_or(ApiError::BadRequest("invalid user id".into()))?
        ;

    let id = crate::types::new_id();
    let mut tdb = state.toasty.clone();
    perm_repo::create(
        &mut tdb,
        id,
        node,
        user,
        body.permission,
        crate::auth::service::now_ms(),
    )
    .await
    .map_err(toasty_err)?;

    let created = perm_repo::get_by_id(&mut tdb, id)
        .await
        .map_err(toasty_err)?
        .ok_or(ApiError::Internal(Box::new(std::io::Error::other(
            "created permission missing",
        ))))?;
    Ok(ApiOk::ok(created))
}

#[derive(Debug, Deserialize)]
pub struct UpdatePermissionBody {
    #[serde(default)]
    pub permission: i32,
}

/// PATCH /nodes/:nodeId/permissions/:permId
pub async fn update_permission(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((node_id, perm_id)): Path<(String, String)>,
    Json(body): Json<UpdatePermissionBody>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let _ = node_id;
    let perm = resolve(&perm_id)?;
    if !(PERM_READ..=PERM_ADMIN).contains(&body.permission) {
        return Err(ApiError::BadRequest("invalid permission level".into()));
    }

    let mut db = state.toasty.clone();
    let _ = &mut db;
    let mut tdb = state.toasty.clone();
    let existing = perm_repo::get_by_id(&mut tdb, perm)
        .await
        .map_err(toasty_err)?
        .ok_or(ApiError::NotFound("not found".into()))?;

    let actor: Actor = auth.into();
    guard::require_node_action(
        &state.db,
        &actor,
        existing.node_id,
        NodeAction::ManagePermissions,
    )
    .await?;

    let mut tdb = state.toasty.clone();
    perm_repo::update(&mut tdb, perm, body.permission)
        .await
        .map_err(toasty_err)?;
    Ok(ApiOk::ok(json!("Permission updated")))
}

/// DELETE /nodes/:nodeId/permissions/:permId — manage-permissions guard OR the
/// permission owner may revoke their own access (Go parity).
pub async fn delete_permission(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((node_id, perm_id)): Path<(String, String)>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let _ = node_id;
    let actor: Actor = auth.into();
    let perm = resolve(&perm_id)?;

    let mut tdb = state.toasty.clone();
    let existing = perm_repo::get_by_id(&mut tdb, perm)
        .await
        .map_err(toasty_err)?
        .ok_or(ApiError::NotFound("not found".into()))?;

    let guard_ok = guard::require_node_action(
        &state.db,
        &actor,
        existing.node_id,
        NodeAction::ManagePermissions,
    )
    .await;
    if guard_ok.is_err() && existing.user_id != actor.user_id {
        return Err(ApiError::Forbidden("forbidden".into()));
    }

    let mut tdb = state.toasty.clone();
    perm_repo::delete(&mut tdb, perm)
        .await
        .map_err(toasty_err)?;
    Ok(ApiOk::ok(json!("Permission deleted")))
}

/// GET /nodes/:nodeId/invitations
pub async fn get_node_invitations(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(node_id): Path<String>,
) -> Result<ApiOk<Vec<serde_json::Value>>, ApiError> {
    let actor: Actor = auth.into();
    let id = resolve(&node_id)?;
    guard::require_node_action(&state.db, &actor, id, NodeAction::ManagePermissions).await?;
    let rows = perm_repo::invitations_by_node(&state.db, id).await?;
    Ok(ApiOk::ok(
        rows.iter().map(invitation_json).collect::<Vec<_>>(),
    ))
}

#[derive(Debug, Deserialize)]
pub struct CreateInvitationBody {
    #[serde(default)]
    pub permission_level: i32,
}

/// POST /nodes/:nodeId/invitations
pub async fn create_invitation(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(node_id): Path<String>,
    Json(body): Json<CreateInvitationBody>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let actor: Actor = auth.into();
    let id = resolve(&node_id)?;
    if !(PERM_READ..=PERM_ADMIN).contains(&body.permission_level) {
        return Err(ApiError::BadRequest(
            "Permission level must be between 1 and 3".into(),
        ));
    }
    guard::require_node_action(&state.db, &actor, id, NodeAction::ManagePermissions).await?;

    let now = crate::auth::service::now_ms();
    let mut last_err: Option<sqlx::Error> = None;
    for _ in 0..10 {
        let code = generate_invitation_code(6);
        let iid = crate::types::new_id();
        match perm_repo::create_invitation(&state.db, iid, &code, body.permission_level, id, now)
            .await
        {
            Ok(()) => {
                return Ok(ApiOk::ok(invitation_json(&(
                    iid,
                    code,
                    body.permission_level,
                    id,
                    now,
                ))));
            }
            Err(e) => {
                // Retry only on unique-constraint collisions (code space).
                let msg = e.to_string().to_lowercase();
                if msg.contains("duplicate") || msg.contains("unique") {
                    last_err = Some(e);
                    continue;
                }
                return Err(e.into());
            }
        }
    }
    let _ = last_err;
    Err(ApiError::BadRequest(
        "failed to generate unique invitation code".into(),
    ))
}

/// DELETE /nodes/:nodeId/invitations/:invitationId
pub async fn delete_invitation(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((node_id, invitation_id)): Path<(String, String)>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let _ = node_id;
    let actor: Actor = auth.into();
    let iid = resolve(&invitation_id)?;

    let row = perm_repo::get_invitation_by_id(&state.db, iid)
        .await
        .map_err(ApiError::from)?
        .ok_or(ApiError::NotFound("not found".into()))?;
    guard::require_node_action(&state.db, &actor, row.3, NodeAction::ManagePermissions).await?;

    perm_repo::delete_invitation(&state.db, iid)
        .await
        .map_err(ApiError::from)?;
    Ok(ApiOk::ok(json!("Invitation deleted")))
}

#[derive(Debug, Deserialize)]
pub struct JoinBody {
    #[serde(default)]
    pub code: String,
}

/// POST /nodes/invitations/join — redeem a code (or invitation URL); upgrades
/// an existing permission when the invitation level is higher.
pub async fn join_invitation(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<JoinBody>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let actor: Actor = auth.into();
    let code =
        parse_invitation_code(&body.code).map_err(|_| ApiError::NotFound("not found".into()))?;

    let row = perm_repo::get_invitation_by_code(&state.db, &code)
        .await
        .map_err(ApiError::from)?
        .ok_or(ApiError::NotFound("not found".into()))?;
    let (level, node_id) = (row.2, row.3);

    let node = perm_repo::node_brief(&state.db, node_id)
        .await
        .map_err(ApiError::from)?
        .ok_or(ApiError::NotFound("not found".into()))?;

    let mut tdb = state.toasty.clone();
    let existing = perm_repo::get_by_node_and_user(&mut tdb, node_id, actor.user_id)
        .await
        .map_err(toasty_err)?;

    let perm = match existing {
        Some(p) if p.permission >= level => p,
        Some(p) => {
            let mut tdb = state.toasty.clone();
            perm_repo::update(&mut tdb, p.id, level)
                .await
                .map_err(toasty_err)?;
            crate::models::PermissionRow {
                id: p.id,
                node_id: p.node_id,
                user_id: p.user_id,
                permission: level,
                created_timestamp: p.created_timestamp,
            }
        }
        None => {
            let pid = crate::types::new_id();
            perm_repo::create(
                &mut tdb,
                pid,
                node_id,
                actor.user_id,
                level,
                crate::auth::service::now_ms(),
            )
            .await
            .map_err(toasty_err)?;
            crate::models::PermissionRow {
                id: pid,
                node_id,
                user_id: actor.user_id,
                permission: level,
                created_timestamp: crate::auth::service::now_ms(),
            }
        }
    };

    Ok(ApiOk::ok(json!({
        "permission": serde_json::to_value(&perm).unwrap_or(json!(null)),
        "node": serde_json::to_value(&node).unwrap_or(json!(null)),
    })))
}
