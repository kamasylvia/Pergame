//! Node access guard (Go `permissions.DefaultAccessGuard` CheckNodeAction /
//! RequireNodeAction parity).
//!
//! Level order: PermNone 0 < PermRead 1 < PermWrite 2 < PermAdmin 3 <
//! PermOwner 4. Owner and app admin get PermOwner; otherwise the
//! ancestor-chain permission (repo::nodes::has_permission) must meet the
//! action's required level.

use axum::http::StatusCode;
use sqlx::PgPool;

use crate::error::ApiError;
use crate::models::NodeRow;
use crate::repo::nodes as node_repo;

pub const PERM_NONE: i32 = 0;
pub const PERM_READ: i32 = 1;
pub const PERM_WRITE: i32 = 2;
pub const PERM_ADMIN: i32 = 3;
pub const PERM_OWNER: i32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Share is part of the Go action set; ported for API completeness
pub enum NodeAction {
    Read,
    Update,
    Delete,
    Share,
    ManagePermissions,
}

impl NodeAction {
    pub fn required_level(self) -> i32 {
        match self {
            Self::Read => PERM_READ,
            Self::Update => PERM_WRITE,
            Self::Delete | Self::Share | Self::ManagePermissions => PERM_ADMIN,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Actor {
    pub user_id: i64,
    pub role: i32,
}

impl From<crate::auth::AuthUser> for Actor {
    fn from(a: crate::auth::AuthUser) -> Self {
        Self {
            user_id: a.user_id(),
            role: a.role,
        }
    }
}

pub fn is_app_admin(role: i32) -> bool {
    role == 2
}

pub fn require_user_access(actor: &Actor, target: i64) -> Result<(), ApiError> {
    if actor.user_id == target || is_app_admin(actor.role) {
        Ok(())
    } else {
        Err(ApiError::Forbidden("forbidden".into()))
    }
}

pub async fn check_node_action(
    pool: &PgPool,
    actor: &Actor,
    node: &NodeRow,
    action: NodeAction,
) -> Result<i32, ApiError> {
    if actor.user_id == node.user_id {
        return Ok(PERM_OWNER);
    }
    if is_app_admin(actor.role) {
        return Ok(PERM_OWNER);
    }
    let (ok, level) =
        node_repo::has_permission(pool, actor.user_id, node.id, action.required_level()).await;
    if ok {
        return Ok(level);
    }
    Err(ApiError::Forbidden("forbidden".into()))
}

/// Fetch + guard in one step (Go RequireNodeAction). Maps missing node to 404
/// "not found".
pub async fn require_node_action(
    pool: &PgPool,
    actor: &Actor,
    node_id: i64,
    action: NodeAction,
) -> Result<NodeRow, ApiError> {
    let node = node_repo::get_by_id(pool, node_id)
        .await
        .map_err(ApiError::from)?
        .ok_or(ApiError::NotFound("not found".into()))?;
    check_node_action(pool, actor, &node, action).await?;
    Ok(node)
}

/// `check_node_action` that surfaces status-less errors like Go (used by the
/// UpdateNode fallback: an error is tolerable when the node is unlisted with
/// access ≥ 2).
pub async fn check_node_action_status(
    pool: &PgPool,
    actor: &Actor,
    node: &NodeRow,
    action: NodeAction,
) -> Result<i32, (StatusCode, ApiError)> {
    check_node_action(pool, actor, node, action)
        .await
        .map_err(|e| (e.status(), e))
}
