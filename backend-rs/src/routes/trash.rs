//! Trash routes (spec .work/features/trash.md): soft-deleted nodes can be
//! listed, restored or purged. Deletion itself stays on DELETE /nodes/:nodeId
//! (now a soft delete); S3 object cleanup only happens on purge/empty, so a
//! restore never finds missing resources.

use axum::extract::{Path, State};
use axum::response::IntoResponse;
use serde_json::json;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::auth::guard::{self, Actor};
use crate::error::ApiError;
use crate::models::NodeRow;
use crate::repo::nodes as node_repo;
use crate::response::ApiOk;

fn resolve_id(param: &str) -> Result<Uuid, ApiError> {
    if param.is_empty() {
        return Err(ApiError::BadRequest("parameter is empty".into()));
    }
    Uuid::parse_str(param)
        .ok_or_else(|| ApiError::BadRequest("invalid parameter".into()))
}

/// Loads a trashed node and enforces owner/app-admin scope. Trashed nodes are
/// invisible to the regular guard (get_by_id filters them out), so ownership is
/// checked directly against the row.
async fn require_trash_node(
    state: &AppState,
    actor: &Actor,
    node_id: Uuid,
) -> Result<(NodeRow, i64), ApiError> {
    let node = node_repo::get_by_id_any(&state.db, node_id)
        .await
        .map_err(ApiError::from)?
        .ok_or(ApiError::NotFound("not found".into()))?;
    let stamp: Option<(Option<i64>,)> =
        sqlx::query_as("SELECT deleted_at FROM nodes WHERE id = $1")
            .bind(node_id)
            .fetch_optional(&state.db)
            .await
            .map_err(ApiError::from)?;
    let deleted_at = stamp
        .and_then(|r| r.0)
        .ok_or_else(|| ApiError::BadRequest("node is not in the trash".into()))?;
    guard::require_user_access(actor, node.user_id)?;
    Ok((node, deleted_at))
}

/// GET /trash — trashed subtree roots of the caller.
pub async fn list_trash(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<impl IntoResponse, ApiError> {
    let actor: Actor = auth.into();
    let nodes = node_repo::list_trash(&state.db, actor.user_id).await?;
    Ok(ApiOk::ok(nodes))
}

/// POST /trash/:nodeId/restore — bring the subtree back.
pub async fn restore_node(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(node_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let actor: Actor = auth.into();
    let id = resolve_id(&node_id)?;
    let (node, deleted_at) = require_trash_node(&state, &actor, id).await?;
    let (restored, reparented) = node_repo::restore_subtree(&state.db, &node, deleted_at).await?;
    Ok(ApiOk::ok(json!({
        "restored": restored,
        "reparented": reparented,
    })))
}

/// DELETE /trash/:nodeId — purge one subtree (this is where S3 objects go).
pub async fn purge_node(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(node_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let actor: Actor = auth.into();
    let id = resolve_id(&node_id)?;
    let _ = require_trash_node(&state, &actor, id).await?;
    let removed = purge_with_storage(&state, id).await?;
    Ok(ApiOk::ok(json!({ "purged": removed })))
}

/// DELETE /trash — empty the caller's trash.
pub async fn empty_trash(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<impl IntoResponse, ApiError> {
    let actor: Actor = auth.into();
    let roots = node_repo::trash_roots(&state.db, actor.user_id).await?;
    let mut purged = 0u64;
    for root in roots {
        purged += purge_with_storage(&state, root).await?;
    }
    Ok(ApiOk::ok(json!({ "purged": purged })))
}

/// Storage cleanup + physical delete of a trashed subtree. S3 failures are
/// logged and ignored (Go parity: the DB delete still proceeds).
async fn purge_with_storage(state: &AppState, node_id: Uuid) -> Result<u64, ApiError> {
    if let Some(storage) = &state.storage
        && let Ok(resources) = node_repo::descendant_resources(&state.db, node_id).await
    {
        for (res_id, res_user, metadata) in resources {
            storage
                .delete_resource_files(res_user, res_id, metadata.as_ref())
                .await;
        }
    }
    Ok(node_repo::purge_subtree(&state.db, node_id).await?)
}

/// Retention sweep: physically deletes trash older than `retention_days`.
/// Called by the startup ticker in main.rs; `retention_days = 0` disables it.
pub async fn sweep_expired(state: &AppState, retention_days: u64) -> Result<u64, ApiError> {
    if retention_days == 0 {
        return Ok(0);
    }
    let cutoff = node_repo::now_ms() - (retention_days as i64) * 86_400_000;
    let rows: Vec<(i64,)> = sqlx::query_as(
        r#"
        SELECT id FROM nodes n
        WHERE n.deleted_at IS NOT NULL AND n.deleted_at < $1
          AND NOT EXISTS (
              SELECT 1 FROM nodes p
              WHERE p.id = n.parent_id AND p.deleted_at IS NOT NULL
          )"#,
    )
    .bind(cutoff)
    .fetch_all(&state.db)
    .await
    .map_err(ApiError::from)?;

    let mut purged = 0u64;
    for (root,) in rows {
        purged += purge_with_storage(state, root).await?;
    }
    Ok(purged)
}
