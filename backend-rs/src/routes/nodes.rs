//! /nodes routes — CRUD, tree listing, shared listing, public view, search
//! (Go controllers/nodes.controller.go + services/node.service.go parity).

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;
use serde_json::json;

use crate::AppState;
use crate::auth::AuthUser;
use crate::auth::guard::{self, Actor, NodeAction};
use crate::error::ApiError;
use crate::repo::nodes as node_repo;
use crate::response::ApiOk;
use crate::utils::sanitize_html;

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    #[serde(default)]
    pub q: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub limit: Option<i32>,
}

/// GET /nodes/search
pub async fn search(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<SearchQuery>,
) -> Result<impl IntoResponse, ApiError> {
    if q.q.chars().count() < 2 {
        return Err(ApiError::BadRequest(
            "Search query must be at least 2 characters".into(),
        ));
    }
    let include_content = q.content == "true";
    let limit = q.limit.unwrap_or(20);
    let results =
        node_repo::search(&state.db, auth.user_id(), &q.q, include_content, limit).await?;
    Ok(ApiOk::ok(results))
}

/// GET /nodes/user/:userId — full tree (self or admin); "@me" = caller.
pub async fn get_user_nodes(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(user_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let caller = auth.user_id();
    let actor: Actor = auth.into();
    let target = resolve_user_param(&user_id, caller)?;
    guard::require_user_access(&actor, target.as_i64())?;
    let nodes = node_repo::list_all(&state.db, target.as_i64()).await?;
    Ok(ApiOk::ok(nodes))
}

/// GET /nodes/shared/:userId — nodes shared with the user (self or admin).
pub async fn get_shared_nodes(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(user_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let caller = auth.user_id();
    let actor: Actor = auth.into();
    let target = resolve_user_param(&user_id, caller)?;
    guard::require_user_access(&actor, target.as_i64())?;
    let nodes = node_repo::list_shared(&state.db, target.as_i64()).await?;
    let ids: Vec<i64> = nodes.iter().map(|n| n.id).collect();
    let perms = node_repo::permissions_for_nodes(&state.db, target.as_i64(), &ids).await?;
    // Attach permissions array to each shared node (Go shape).
    let value: Vec<serde_json::Value> = nodes
        .iter()
        .map(|n| {
            let mut v = serde_json::to_value(n).unwrap_or(json!(null));
            let node_perms: Vec<_> = perms
                .iter()
                .filter(|p| p.node_id == n.id)
                .map(|p| serde_json::to_value(p).unwrap_or(json!(null)))
                .collect();
            if let Some(obj) = v.as_object_mut() {
                obj.insert("permissions".into(), json!(node_perms));
            }
            v
        })
        .collect();
    Ok(ApiOk::ok(value))
}

/// GET /nodes/:nodeId — node + filtered permissions (Go GetNode shape).
pub async fn get_node(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(node_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let actor: Actor = auth.into();
    let id = resolve_id(&node_id)?.as_i64();
    let node = guard::require_node_action(&state.db, &actor, id, NodeAction::Read).await?;
    let perms = node_repo::permissions_for_nodes(&state.db, actor.user_id, &[id]).await?;
    let owner = node.user_id == actor.user_id;
    let filtered: Vec<_> = perms
        .iter()
        .filter(|p| p.user_id == actor.user_id || owner)
        .map(|p| serde_json::to_value(p).unwrap_or(json!(null)))
        .collect();
    Ok(ApiOk::ok(json!({
        "node": serde_json::to_value(&node).unwrap_or(json!(null)),
        "permissions": filtered,
    })))
}

/// GET /nodes/public/:nodeId — public subtree (no auth).
pub async fn get_public_node(
    State(state): State<AppState>,
    Path(node_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let id = resolve_id(&node_id)?.as_i64();
    let node = node_repo::get_public(&state.db, id).await?;
    if node.is_none() {
        // Go: nil service result marshals to `null`.
        return Ok(ApiOk::ok(serde_json::Value::Null));
    }
    let node = node.unwrap();
    let children = if node.role == 1 || node.role == 2 || node.role == 3 {
        node_repo::public_descendants(&state.db, id).await?
    } else {
        vec![]
    };
    Ok(ApiOk::ok(json!({
        "node": serde_json::to_value(&node).unwrap_or(json!(null)),
        "children": children,
    })))
}

#[derive(Debug, Deserialize)]
pub struct NodeInput {
    #[serde(default, with = "crate::models::sf_opt")]
    pub parent_id: Option<i64>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Option<String>,
    #[serde(default)]
    pub role: i32,
    #[serde(default)]
    pub color: Option<i32>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub thumbnail: Option<String>,
    #[serde(default)]
    pub theme: Option<String>,
    #[serde(default)]
    pub accessibility: Option<i32>,
    #[serde(default)]
    pub access: i32,
    #[serde(default)]
    pub display: Option<i32>,
    #[serde(default)]
    pub order: Option<i32>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub content_compiled: Option<String>,
    #[serde(default)]
    pub size: Option<i64>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

impl NodeInput {
    fn into_write(self) -> node_repo::NodeWrite {
        node_repo::NodeWrite {
            parent_id: self.parent_id,
            name: self.name,
            description: self.description,
            tags: self.tags,
            role: self.role,
            color: self.color,
            icon: Some(sanitize_html(self.icon.as_deref())),
            thumbnail: Some(sanitize_html(self.thumbnail.as_deref())),
            theme: self.theme,
            accessibility: self.accessibility,
            access: self.access,
            display: self.display,
            node_order: self.order,
            content: self.content.clone(),
            content_compiled: Some(sanitize_html(self.content_compiled.as_deref())),
            size: node_size(self.role, self.size, &self.content, &self.content_compiled),
            metadata: self.metadata,
        }
    }
}

/// Go getNodeSize: file nodes keep the provided size; others derive it from
/// content lengths.
fn node_size(
    role: i32,
    size: Option<i64>,
    content: &Option<String>,
    compiled: &Option<String>,
) -> Option<i64> {
    if role == 4 && size.is_some() {
        return size;
    }
    let mut total = 0i64;
    if let Some(c) = compiled {
        total += c.len() as i64;
    }
    if let Some(c) = content {
        total += c.len() as i64;
    }
    Some(total)
}

fn resolve_id(param: &str) -> Result<crate::types::Snowflake, ApiError> {
    if param.is_empty() {
        return Err(ApiError::BadRequest("parameter is empty".into()));
    }
    crate::types::Snowflake::parse(param)
        .ok_or_else(|| ApiError::BadRequest("invalid parameter".into()))
}

/// User-scoped path param: "@me" resolves to the authenticated caller
/// (frontend contract `nodes/user/@me`, Go controller parity).
fn resolve_user_param(param: &str, caller: i64) -> Result<crate::types::Snowflake, ApiError> {
    if param == "@me" {
        return Ok(crate::types::Snowflake(caller as u64));
    }
    resolve_id(param)
}

/// POST /nodes — create under the actor.
pub async fn create_node(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<NodeInput>,
) -> Result<impl IntoResponse, ApiError> {
    let actor: Actor = auth.into();
    if input.name.is_empty() {
        return Err(ApiError::BadRequest(
            "Key: 'Node.Name' Error: field validation failed".into(),
        ));
    }

    let mut safe_parent = input.parent_id;
    if let Some(pid) = safe_parent {
        let parent = node_repo::get_by_id(&state.db, pid).await.unwrap_or(None);
        let allowed = match parent {
            Some(p) => guard::check_node_action(&state.db, &actor, &p, NodeAction::Update)
                .await
                .map(|l| l != guard::PERM_NONE)
                .unwrap_or(false),
            None => false,
        };
        if !allowed {
            safe_parent = None;
        }
    }

    let mut write = input.into_write();
    write.parent_id = safe_parent;

    let id = state.snowflake.generate().as_i64();
    let now = node_repo::now_ms();
    node_repo::insert(&state.db, id, actor.user_id, &write, now)
        .await
        .map_err(ApiError::from)?;

    let created = node_repo::get_by_id(&state.db, id)
        .await
        .map_err(ApiError::from)?
        .ok_or(ApiError::Internal(Box::new(std::io::Error::other(
            "created node missing",
        ))))?;
    Ok(ApiOk::with_status(StatusCode::OK, created))
}

/// PUT /nodes/:nodeId — update (write-level access; unlisted+access≥2 open
/// fallback; non-owner edits keep ownership/security fields).
pub async fn update_node(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(node_id): Path<String>,
    Json(input): Json<NodeInput>,
) -> Result<impl IntoResponse, ApiError> {
    let actor: Actor = auth.into();
    let id = resolve_id(&node_id)?.as_i64();

    let db_node = node_repo::get_by_id(&state.db, id)
        .await
        .map_err(ApiError::from)?
        .ok_or(ApiError::NotFound("not found".into()))?;

    let mut parent = input.parent_id;
    let mut user_id = actor.user_id;
    let mut accessibility = input.accessibility;
    let mut access = input.access;

    let level =
        guard::check_node_action_status(&state.db, &actor, &db_node, NodeAction::Update).await;
    let level = match level {
        Ok(l) => Some(l),
        Err((_, err)) => {
            let bypass = db_node.access >= 2
                && db_node.accessibility.is_some()
                && db_node.accessibility.unwrap() == 3;
            if !bypass {
                return Err(err);
            }
            // Open-unlisted edits behave like write access without ownership.
            None
        }
    };

    let is_owner_edit =
        db_node.user_id == actor.user_id || level.map(|l| l >= guard::PERM_OWNER).unwrap_or(false);
    if !is_owner_edit {
        parent = db_node.parent_id;
        user_id = db_node.user_id;
        accessibility = db_node.accessibility;
        access = db_node.access;
    }

    let mut safe_parent = parent;
    if let Some(pid) = safe_parent {
        let parent_row = node_repo::get_by_id(&state.db, pid).await.unwrap_or(None);
        let allowed = match parent_row {
            Some(p) => guard::check_node_action(&state.db, &actor, &p, NodeAction::Update)
                .await
                .map(|l| l != guard::PERM_NONE)
                .unwrap_or(false),
            None => false,
        };
        if !allowed {
            safe_parent = None;
        }
    }

    let mut write = input.into_write();
    write.parent_id = safe_parent;
    write.accessibility = accessibility;
    write.access = access;

    node_repo::update(&state.db, id, &write, node_repo::now_ms())
        .await
        .map_err(ApiError::from)?;
    let _ = user_id; // Go updates user_id via repo.Update omitting it; node ownership
    // only changes implicitly through parent moves — kept identical.

    let updated = node_repo::get_by_id(&state.db, id)
        .await
        .map_err(ApiError::from)?
        .ok_or(ApiError::NotFound("not found".into()))?;
    Ok(ApiOk::ok(updated))
}

/// DELETE /nodes/:nodeId — delete-level guard; descendant role=4 files are
/// collected for storage cleanup (S3 client lands with P3.5; deletion itself
/// matches the Go DB cascade).
pub async fn delete_node(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(node_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let actor: Actor = auth.into();
    let id = resolve_id(&node_id)?.as_i64();
    let node = guard::require_node_action(&state.db, &actor, id, NodeAction::Delete).await?;

    // Storage cleanup: descendant role=4 files are removed before the DB
    // cascade (Go parity: failures logged and ignored).
    if let Some(storage) = &state.storage
        && let Ok(resources) = node_repo::descendant_resources(&state.db, node.id).await
    {
        for (res_id, res_user, metadata) in resources {
            storage
                .delete_resource_files(res_user, res_id, metadata.as_ref())
                .await;
        }
    }

    node_repo::delete(&state.db, node.id)
        .await
        .map_err(ApiError::from)?;
    Ok(ApiOk::ok(json!("OK")))
}
