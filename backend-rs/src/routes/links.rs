//! Node soft-link routes (spec .work/features/note-links.md):
//! POST /nodes/:id/links — clone into a folder (cycle + permission guards)
//! DELETE /nodes/:id/links/:linkId — remove one link position
//! GET /nodes/:id/links — list all positions of the note

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::json;

use crate::AppState;
use crate::auth::AuthUser;
use crate::auth::guard::{self, Actor, NodeAction};
use crate::error::ApiError;
use crate::models::LinkRow;
use crate::repo::links as link_repo;
use crate::response::ApiOk;

/// Toasty errors arrive as plain strings (the repo layer does not depend on
/// ApiError); map them to the envelope here.
fn toasty_err(e: String) -> ApiError {
    ApiError::Internal(Box::new(std::io::Error::other(e)))
}
use uuid::Uuid;

fn resolve(param: &str) -> Result<Uuid, ApiError> {
    if param.is_empty() {
        return Err(ApiError::BadRequest("parameter is empty".into()));
    }
    Uuid::parse_str(param).ok_or_else(|| ApiError::BadRequest("invalid parameter".into()))
}

#[derive(Debug, Deserialize)]
pub struct CreateLinkBody {
    #[serde(default)]
    pub parent_id: String,
}

/// POST /nodes/:nodeId/links — clone the note into another folder.
/// Guards: Update on the note (its content moves visibility), Update on the
/// target parent; the target may not be the note itself or its subtree.
pub async fn create_link(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(node_id): Path<String>,
    Json(body): Json<CreateLinkBody>,
) -> Result<ApiOk<LinkRow>, ApiError> {
    let actor: Actor = auth.into();
    let node = resolve(&node_id)?;

    let parent = Uuid::parse_str(&body.parent_id)
        .ok_or(ApiError::BadRequest("invalid parent_id".into()))?
        ;
    if parent == node {
        return Err(ApiError::BadRequest(
            "cannot link a note into itself".into(),
        ));
    }

    let parent_row = node_repo_get(&state.db, parent)
        .await
        .ok_or(ApiError::NotFound("target parent not found".into()))?;
    guard::check_node_action(&state.db, &actor, &parent_row, NodeAction::Update).await?;

    // Cycle rule: the target parent may not be the note or its subtree.
    if crate::repo::links::is_self_or_descendant(&state.db, node, parent)
        .await
        .map_err(ApiError::from)?
    {
        return Err(ApiError::BadRequest(
            "cannot link a note into itself or its subtree".into(),
        ));
    }

    let id = crate::types::new_id();
    let mut tdb = state.toasty.clone();
    link_repo::create(
        &mut tdb,
        id,
        parent,
        node,
        0,
        crate::auth::service::now_ms(),
    )
    .await
    .map_err(|e| {
        let msg = e.to_string();
        if msg.contains("duplicate key") || msg.contains("23505") {
            return ApiError::Conflict("link already exists in that folder".into());
        }
        toasty_err(e)
    })?;

    let row = link_repo::get_by_id(&mut tdb, id)
        .await
        .map_err(toasty_err)?
        .ok_or(ApiError::Internal(Box::new(std::io::Error::other(
            "created link missing",
        ))))?;
    Ok(ApiOk::with_status(StatusCode::CREATED, row))
}

/// DELETE /nodes/:nodeId/links/:linkId — remove one link position.
/// Guard: Update on the note (removing a viewing position needs it) OR Update
/// on the link's parent (folder owner tidying their folder).
pub async fn delete_link(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((node_id, link_id)): Path<(String, String)>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let actor: Actor = auth.into();
    let node = resolve(&node_id)?;
    let link = resolve(&link_id)?;

    let note_row = node_repo_get(&state.db, node)
        .await
        .ok_or(ApiError::NotFound("node not found".into()))?;
    // Position removal needs write on the note or write on the parent folder.
    let mut tdb = state.toasty.clone();
    let row = link_repo::get_by_id(&mut tdb, link)
        .await
        .map_err(toasty_err)?
        .ok_or(ApiError::NotFound("link not found".into()))?;
    let note_ok = guard::check_node_action(&state.db, &actor, &note_row, NodeAction::Update)
        .await
        .is_ok();
    let parent_row = node_repo_get(&state.db, row.parent_id).await;
    let parent_ok = match parent_row.as_ref() {
        Some(p) => guard::check_node_action(&state.db, &actor, p, NodeAction::Update)
            .await
            .is_ok(),
        None => false,
    };
    if !note_ok && !parent_ok {
        return Err(ApiError::Forbidden("forbidden".into()));
    }

    let mut tdb = state.toasty.clone();
    let deleted = link_repo::delete(&mut tdb, link)
        .await
        .map_err(toasty_err)?;
    if !deleted {
        return Err(ApiError::NotFound("link not found".into()));
    }
    Ok(ApiOk::ok(json!({ "message": "Link removed" })))
}

/// GET /nodes/:nodeId/links — all positions of the note.
pub async fn get_links(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(node_id): Path<String>,
) -> Result<ApiOk<Vec<LinkRow>>, ApiError> {
    let actor: Actor = auth.into();
    let node = resolve(&node_id)?;
    let _ = guard::require_node_action(&state.db, &actor, node, NodeAction::Read).await?;
    let mut tdb = state.toasty.clone();
    let rows = link_repo::links_of_node(&mut tdb, node)
        .await
        .map_err(toasty_err)?;
    Ok(ApiOk::ok(rows))
}

async fn node_repo_get(pool: &sqlx::PgPool, id: Uuid) -> Option<crate::models::NodeRow> {
    crate::repo::nodes::get_by_id(pool, id).await.ok().flatten()
}

/// GET /nodes/links — every soft-link position anchored on the caller's own
/// nodes (the sidebar tree consumes this to render clone positions).
pub async fn list_user_links(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<ApiOk<Vec<LinkRow>>, ApiError> {
    let rows = link_repo::list_by_user(&state.db, auth.user_id())
        .await
        .map_err(ApiError::from)?;
    Ok(ApiOk::ok(rows))
}
