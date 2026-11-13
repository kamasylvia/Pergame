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

/// Permission levels (ascending): no access.
pub const PERM_NONE: i32 = 0;
/// Permission levels (ascending): read.
pub const PERM_READ: i32 = 1;
/// Permission levels (ascending): write.
pub const PERM_WRITE: i32 = 2;
/// Permission levels (ascending): manage subtree.
pub const PERM_ADMIN: i32 = 3;
/// Permission levels (ascending): owner / app admin.
pub const PERM_OWNER: i32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Share is part of the Go action set; ported for API completeness
/// Node operation being authorized.
pub enum NodeAction {
    Read,
    Update,
    Delete,
    Share,
    ManagePermissions,
}

impl NodeAction {
    /// Minimum permission level this action requires.
    pub fn required_level(self) -> i32 {
        match self {
            Self::Read => PERM_READ,
            Self::Update => PERM_WRITE,
            Self::Delete | Self::Share | Self::ManagePermissions => PERM_ADMIN,
        }
    }
}

#[derive(Debug, Clone)]
/// The user + role requesting access.
pub struct Actor {
    pub user_id: uuid::Uuid,
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

/// True when the role is app admin (`role == 2`).
pub fn is_app_admin(role: i32) -> bool {
    role == 2
}

/// Ok when acting on self or as app admin, else 403.
pub fn require_user_access(actor: &Actor, target: uuid::Uuid) -> Result<(), ApiError> {
    if actor.user_id == target || is_app_admin(actor.role) {
        Ok(())
    } else {
        Err(ApiError::Forbidden("forbidden".into()))
    }
}

/// Resolve the actor's effective level on a node (owner/admin short-circuit).
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
    node_id: uuid::Uuid,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_levels_match_go() {
        assert_eq!(NodeAction::Read.required_level(), PERM_READ);
        assert_eq!(NodeAction::Update.required_level(), PERM_WRITE);
        assert_eq!(NodeAction::Delete.required_level(), PERM_ADMIN);
        assert_eq!(NodeAction::Share.required_level(), PERM_ADMIN);
        assert_eq!(NodeAction::ManagePermissions.required_level(), PERM_ADMIN);
        assert!(PERM_NONE < PERM_READ && PERM_READ < PERM_WRITE);
        assert!(PERM_WRITE < PERM_ADMIN && PERM_ADMIN < PERM_OWNER);
    }

    #[test]
    fn self_or_admin_passes() {
        let me = uuid::Uuid::now_v7();
        let actor = Actor {
            user_id: me,
            role: 1,
        };
        assert!(require_user_access(&actor, me).is_ok());
        assert!(require_user_access(&actor, uuid::Uuid::now_v7()).is_err());
        let admin = Actor {
            user_id: uuid::Uuid::now_v7(),
            role: 2,
        };
        assert!(require_user_access(&admin, me).is_ok());
        assert!(is_app_admin(2));
        assert!(!is_app_admin(1));
    }
}
