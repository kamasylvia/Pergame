//! PDF annotation routes (SiYuan-style: rows in Pergame, file bytes clean):
//! GET/POST /resources/:id/annotations — list/create on a PDF resource
//! PATCH/DELETE /annotations/:id — edit comment+color / remove
//! GET /annotations/:id/refs — docs linking to one annotation

use axum::Json;
use axum::extract::{Path, State};
use serde::Deserialize;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::auth::guard::{self, Actor, NodeAction};
use crate::error::ApiError;
use crate::repo::annotations::{self as ann_repo, Annotation, Quad};
use crate::response::ApiOk;

fn resolve(param: &str) -> Result<Uuid, ApiError> {
    if param.is_empty() {
        return Err(ApiError::BadRequest("parameter is empty".into()));
    }
    Uuid::parse_str(param).map_err(|_| ApiError::BadRequest("invalid parameter".into()))
}

async fn resource_row(state: &AppState, id: Uuid) -> Result<crate::models::NodeRow, ApiError> {
    let row = crate::repo::nodes::get_by_id(&state.db, id)
        .await
        .map_err(ApiError::from)?
        .ok_or(ApiError::NotFound("resource not found".into()))?;
    if row.role != 4 {
        return Err(ApiError::BadRequest("not a resource".into()));
    }
    Ok(row)
}

#[derive(Debug, Deserialize)]
/// POST /resources/:id/annotations body.
pub struct CreateAnnotationBody {
    pub page: i32,
    pub quads: Vec<Quad>,
    #[serde(default)]
    pub quote: String,
    #[serde(default)]
    pub comment: String,
    #[serde(default)]
    pub color: i32,
}

/// GET /resources/:id/annotations — reading order. Read gate on the file.
pub async fn list_annotations(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<ApiOk<Vec<Annotation>>, ApiError> {
    let actor: Actor = auth.into();
    let row = resource_row(&state, resolve(&id)?).await?;
    guard::check_node_action(&state.db, &actor, &row, NodeAction::Read).await?;
    let rows = ann_repo::list_for_resource(&state.db, row.id)
        .await
        .map_err(ApiError::from)?;
    Ok(ApiOk::ok(rows))
}

/// GET /annotations/:id — single row (deep links). Read gate on the file.
pub async fn get_annotation(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<ApiOk<Annotation>, ApiError> {
    let actor: Actor = auth.into();
    let ann = ann_repo::get(&state.db, resolve(&id)?)
        .await
        .map_err(ApiError::from)?
        .ok_or(ApiError::NotFound("annotation not found".into()))?;
    let row = resource_row(&state, ann.resource_id).await?;
    guard::check_node_action(&state.db, &actor, &row, NodeAction::Read).await?;
    Ok(ApiOk::ok(ann))
}
/// POST /resources/:id/annotations — Update gate on the file (same bar as
/// renaming it); annotations are personal rows, the bytes never change.
pub async fn create_annotation(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<CreateAnnotationBody>,
) -> Result<ApiOk<Annotation>, ApiError> {
    let actor: Actor = auth.into();
    let row = resource_row(&state, resolve(&id)?).await?;
    guard::check_node_action(&state.db, &actor, &row, NodeAction::Update).await?;
    let filetype = row
        .metadata
        .as_ref()
        .and_then(|m| m.get("filetype"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    if filetype != "application/pdf" {
        return Err(ApiError::BadRequest(
            "annotations need a PDF resource".into(),
        ));
    }
    if body.page.lt(&1) {
        return Err(ApiError::BadRequest("page must be >= 1".into()));
    }
    if !(0..8).contains(&body.color) {
        return Err(ApiError::BadRequest("color must be 0..=7".into()));
    }
    if body.quote.chars().count().gt(&2000) {
        return Err(ApiError::BadRequest("quote too long (2000 chars)".into()));
    }
    if body.comment.chars().count().gt(&5000) {
        return Err(ApiError::BadRequest("comment too long (5000 chars)".into()));
    }
    let quads = ann_repo::checked_quads(&body.quads).map_err(ApiError::BadRequest)?;
    let now = crate::auth::service::now_ms();
    let ann = ann_repo::create(
        &state.db,
        ann_repo::NewAnnotation {
            id: crate::types::new_id(),
            user_id: actor.user_id,
            resource_id: row.id,
            page: body.page,
            quads,
            quote: body.quote.trim(),
            comment: body.comment.trim(),
            color: body.color,
            now,
        },
    )
    .await
    .map_err(ApiError::from)?;
    Ok(ApiOk::ok(ann))
}

#[derive(Debug, Deserialize)]
/// PATCH /annotations/:id body (geometry is immutable; re-create to move).
pub struct UpdateAnnotationBody {
    #[serde(default)]
    pub comment: Option<String>,
    #[serde(default)]
    pub color: Option<i32>,
}

/// Load + gate: the creator or anyone with Update on the file.
async fn owned_or_writable(
    state: &AppState,
    actor: &Actor,
    id: Uuid,
) -> Result<Annotation, ApiError> {
    let ann = ann_repo::get(&state.db, id)
        .await
        .map_err(ApiError::from)?
        .ok_or(ApiError::NotFound("annotation not found".into()))?;
    if ann.user_id == actor.user_id {
        return Ok(ann);
    }
    let row = resource_row(state, ann.resource_id).await?;
    guard::check_node_action(&state.db, actor, &row, NodeAction::Update).await?;
    Ok(ann)
}

/// PATCH /annotations/:id — comment/color only.
pub async fn update_annotation(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<UpdateAnnotationBody>,
) -> Result<ApiOk<Annotation>, ApiError> {
    let actor: Actor = auth.into();
    let ann = owned_or_writable(&state, &actor, resolve(&id)?).await?;
    let comment = body.comment.as_deref().unwrap_or(&ann.comment);
    if comment.chars().count().gt(&5000) {
        return Err(ApiError::BadRequest("comment too long (5000 chars)".into()));
    }
    let color = body.color.unwrap_or(ann.color);
    if !(0..8).contains(&color) {
        return Err(ApiError::BadRequest("color must be 0..=7".into()));
    }
    let now = crate::auth::service::now_ms();
    let updated = ann_repo::update(&state.db, ann.id, comment.trim(), color, now)
        .await
        .map_err(ApiError::from)?
        .ok_or(ApiError::NotFound("annotation not found".into()))?;
    Ok(ApiOk::ok(updated))
}

/// DELETE /annotations/:id — refs cascade.
pub async fn delete_annotation(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let actor: Actor = auth.into();
    let ann = owned_or_writable(&state, &actor, resolve(&id)?).await?;
    ann_repo::delete(&state.db, ann.id)
        .await
        .map_err(ApiError::from)?;
    Ok(ApiOk::ok(serde_json::json!({"deleted": true})))
}

/// GET /annotations/:id/refs — docs linking here (Read gate on the file,
/// sources visibility-filtered).
pub async fn annotation_refs(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<ApiOk<Vec<crate::repo::nodes::refs::RefNode>>, ApiError> {
    let actor: Actor = auth.into();
    let ann = ann_repo::get(&state.db, resolve(&id)?)
        .await
        .map_err(ApiError::from)?
        .ok_or(ApiError::NotFound("annotation not found".into()))?;
    let row = resource_row(&state, ann.resource_id).await?;
    guard::check_node_action(&state.db, &actor, &row, NodeAction::Read).await?;
    let rows = ann_repo::refs(&state.db, actor.user_id, ann.id)
        .await
        .map_err(ApiError::from)?;
    Ok(ApiOk::ok(rows))
}
