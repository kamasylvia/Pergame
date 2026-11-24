//! /nodes routes — CRUD, tree listing, shared listing, public view, search
//! (Go controllers/nodes.controller.go + services/node.service.go parity).

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::auth::guard::{self, Actor, NodeAction};
use crate::error::ApiError;
use crate::repo::nodes as node_repo;
use crate::response::ApiOk;
use crate::utils::sanitize_html;

#[derive(Debug, Deserialize)]
/// GET /nodes/search query (q/content/limit).
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

/// GET /nodes/containers — the caller's workspaces and folders, flat.
///
/// Pickers (plugin dialogs, move targets) need names and ids, not the whole
/// tree: `list_all` walks every node recursively, which costs seconds on a
/// large library.
pub async fn list_containers(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<impl IntoResponse, ApiError> {
    let nodes = node_repo::list_containers(&state.db, auth.user_id()).await?;
    Ok(ApiOk::ok(nodes))
}

#[derive(Debug, Deserialize)]
/// GET /nodes/roots + /nodes/:id/children paging (`offset`/`limit`), tree
/// sort (`sort=name|created|modified`, `order=asc|desc`, preference parity)
/// and optional single-role narrowing (`role=0..4`).
pub struct ChildrenQuery {
    #[serde(default)]
    pub offset: Option<i64>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub sort: Option<String>,
    #[serde(default)]
    pub order: Option<String>,
    #[serde(default)]
    pub role: Option<i32>,
}

fn parse_role(role: Option<i32>) -> Result<Option<i32>, ApiError> {
    match role {
        None => Ok(None),
        Some(r) if (0..=4).contains(&r) => Ok(Some(r)),
        Some(_) => Err(ApiError::BadRequest("role must be 0..4".into())),
    }
}

fn parse_page(
    q: &ChildrenQuery,
) -> Result<
    (
        i64,
        i64,
        node_repo::tree::TreeSortKey,
        node_repo::tree::SortDir,
    ),
    ApiError,
> {
    let offset = q.offset.unwrap_or(0).max(0);
    let limit = q.limit.unwrap_or(100).clamp(1, 500);
    let key = node_repo::tree::TreeSortKey::parse(q.sort.as_deref().unwrap_or("name"))
        .ok_or_else(|| ApiError::BadRequest("sort must be name|created|modified".into()))?;
    let dir = node_repo::tree::SortDir::parse(q.order.as_deref().unwrap_or("asc"))
        .ok_or_else(|| ApiError::BadRequest("order must be asc|desc".into()))?;
    Ok((offset, limit, key, dir))
}

/// GET /nodes/roots — top-level rows for lazy tree bootstrap (owned
/// workspaces/teams and parentless nodes, plus directly granted nodes the
/// frontend roots when their parent is not loaded). Optional `role` narrows
/// to one role (workspace switchers call `role=1`).
pub async fn list_roots(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<ChildrenQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let (offset, limit, key, dir) = parse_page(&q)?;
    let role = parse_role(q.role)?;
    let (rows, total) =
        node_repo::tree::roots_page(&state.db, auth.user_id(), role, offset, limit, key, dir)
            .await
            .map_err(ApiError::from)?;
    Ok(ApiOk::ok(
        json!({ "rows": rows, "total": total, "offset": offset, "limit": limit }),
    ))
}

/// GET /nodes/:id/children — one visible directory page (canonical children
/// only; clone positions stay on the links set), `has_children` per row so
/// chevrons render without fetching. Optional `role` narrows. Read access
/// on the parent is required.
pub async fn list_children(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(node_id): Path<String>,
    Query(q): Query<ChildrenQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let actor: Actor = auth.into();
    let id = resolve_id(&node_id)?;
    guard::require_node_action(&state.db, &actor, id, NodeAction::Read).await?;
    let (offset, limit, key, dir) = parse_page(&q)?;
    let role = parse_role(q.role)?;
    let (rows, total) =
        node_repo::tree::children_page(&state.db, actor.user_id, id, role, offset, limit, key, dir)
            .await
            .map_err(ApiError::from)?;
    Ok(ApiOk::ok(
        json!({ "rows": rows, "total": total, "offset": offset, "limit": limit }),
    ))
}

/// GET /nodes/:id/ancestors — chain leaf-first (`[self … root]`) for
/// breadcrumbs, reveal and permission walks. Read access required.
pub async fn list_ancestors(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(node_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let actor: Actor = auth.into();
    let id = resolve_id(&node_id)?;
    guard::require_node_action(&state.db, &actor, id, NodeAction::Read).await?;
    let chain = node_repo::tree::ancestors(&state.db, id)
        .await
        .map_err(ApiError::from)?;
    Ok(ApiOk::ok(chain))
}

#[derive(Debug, Deserialize)]
/// GET /nodes/flat paging + filters (`role`, `root` subtree id, `pinned`).
pub struct FlatQuery {
    #[serde(default)]
    pub offset: Option<i64>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub sort: Option<String>,
    #[serde(default)]
    pub order: Option<String>,
    #[serde(default)]
    pub role: Option<i32>,
    #[serde(default)]
    pub root: Option<String>,
    #[serde(default)]
    pub pinned: Option<bool>,
}

/// GET /nodes/flat — paged flat list over visible rows (index pages, CDN
/// listings, cards), optionally scoped to one role and/or one subtree.
pub async fn list_flat(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<FlatQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let page = ChildrenQuery {
        offset: q.offset,
        limit: q.limit,
        sort: q.sort.clone(),
        order: q.order.clone(),
        role: q.role,
    };
    let (offset, limit, key, dir) = parse_page(&page)?;
    let role = parse_role(q.role)?;
    let root = match q.root.as_deref() {
        None => None,
        Some(raw) => Some(resolve_id(raw)?),
    };
    let (rows, total) = node_repo::lists::flat_page(
        &state.db,
        auth.user_id(),
        role,
        root,
        q.pinned.unwrap_or(false),
        offset,
        limit,
        key,
        dir,
    )
    .await
    .map_err(ApiError::from)?;
    Ok(ApiOk::ok(
        json!({ "rows": rows, "total": total, "offset": offset, "limit": limit }),
    ))
}

/// GET /nodes/stats — counts for stats cards, globally or under one root.
pub async fn node_stats(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<FlatQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let root = match q.root.as_deref() {
        None => None,
        Some(raw) => Some(resolve_id(raw)?),
    };
    let stats = node_repo::lists::stats(&state.db, auth.user_id(), root)
        .await
        .map_err(ApiError::from)?;
    Ok(ApiOk::ok(stats))
}

/// GET /nodes/activity — per-day live-node counts, newest first (home feed).
pub async fn node_activity(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<ChildrenQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let limit = q.limit.unwrap_or(15).clamp(1, 90);
    let days = node_repo::lists::activity(&state.db, auth.user_id(), limit)
        .await
        .map_err(ApiError::from)?;
    Ok(ApiOk::ok(days))
}

/// GET /nodes/tags — distinct raw tag strings for suggesters.
pub async fn node_tags(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<impl IntoResponse, ApiError> {
    let tags = node_repo::lists::distinct_tags(&state.db, auth.user_id())
        .await
        .map_err(ApiError::from)?;
    Ok(ApiOk::ok(tags))
}

/// GET /nodes/:id/neighbors — adjacent visible siblings under one ordering
/// (prev/next navigation without loading the directory). Optional `role`
/// restricts the sibling run (doc pages pass `role=3`).
pub async fn list_neighbors(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(node_id): Path<String>,
    Query(q): Query<ChildrenQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let actor: Actor = auth.into();
    let id = resolve_id(&node_id)?;
    guard::require_node_action(&state.db, &actor, id, NodeAction::Read).await?;
    let (_, _, key, dir) = parse_page(&q)?;
    let role = parse_role(q.role)?;
    let (prev, next) = node_repo::lists::neighbors(&state.db, actor.user_id, id, role, key, dir)
        .await
        .map_err(ApiError::from)?;
    Ok(ApiOk::ok(json!({ "prev": prev, "next": next })))
}

#[derive(Debug, Deserialize)]
/// GET /nodes/search/meta query (substring over name/description/tags).
pub struct MetaSearchQuery {
    pub q: String,
    #[serde(default)]
    pub role: Option<i32>,
    #[serde(default)]
    pub limit: Option<i64>,
}

/// GET /nodes/search/meta — literal substring search over metadata, any
/// length (covers what FTS refuses). Shared-aware, newest first.
pub async fn meta_search(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<MetaSearchQuery>,
) -> Result<impl IntoResponse, ApiError> {
    if q.q.trim().is_empty() {
        return Err(ApiError::BadRequest("q must not be empty".into()));
    }
    let role = parse_role(q.role)?;
    let limit = q.limit.unwrap_or(50).clamp(1, 100);
    let rows = node_repo::lists::meta_search(&state.db, auth.user_id(), q.q.trim(), role, limit)
        .await
        .map_err(ApiError::from)?;
    Ok(ApiOk::ok(rows))
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
    guard::require_user_access(&actor, target)?;
    let nodes = node_repo::list_all(&state.db, target).await?;
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
    guard::require_user_access(&actor, target)?;
    let nodes = node_repo::list_shared(&state.db, target).await?;
    let ids: Vec<Uuid> = nodes.iter().map(|n| n.id).collect();
    let perms = node_repo::permissions_for_nodes(&state.db, target, &ids).await?;
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
    let id = resolve_id(&node_id)?;
    let node = guard::require_node_action(&state.db, &actor, id, NodeAction::Read).await?;
    // Recently-viewed history for `SORT viewed_timestamp` (dataview "最近查看"
    // tables): record this read for the caller. A failed write must never
    // break a read.
    let _ = node_repo::record_view(&state.db, actor.user_id, id, node_repo::now_ms()).await;
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
    let id = resolve_id(&node_id)?;
    let node = node_repo::get_public(&state.db, id).await?;
    let Some(node) = node else {
        // Go: nil service result marshals to `null`.
        return Ok(ApiOk::ok(serde_json::Value::Null));
    };
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

#[derive(Debug, Default, Deserialize)]
/// POST/PUT /nodes body (create or full replace).
pub struct NodeInput {
    #[serde(default)]
    pub parent_id: Option<Uuid>,
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
    /// VARCHAR ceilings from `migrations/0001_init.sql`, mirroring the Go
    /// model's `binding:"required,max=50"` / `omitempty,max=250` tags.
    /// Without this the INSERT/UPDATE dies in the driver and surfaces as an
    /// opaque 500 (seen live: a 51-char clip title).
    fn validate_lengths(&self) -> Result<(), ApiError> {
        for (field, value, max) in [
            ("Name", Some(self.name.as_str()), 50),
            ("Description", self.description.as_deref(), 250),
            ("Tags", self.tags.as_deref(), 250),
        ] {
            if value.map(|v| v.chars().count()).unwrap_or(0) > max {
                return Err(ApiError::BadRequest(format!(
                    "Key: 'Node.{field}' Error: field validation for '{field}' failed on the 'max' tag"
                )));
            }
        }
        Ok(())
    }

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

fn resolve_id(param: &str) -> Result<Uuid, ApiError> {
    if param.is_empty() {
        return Err(ApiError::BadRequest("parameter is empty".into()));
    }
    Uuid::parse_str(param).map_err(|_| ApiError::BadRequest("invalid parameter".into()))
}

/// User-scoped path param: "@me" resolves to the authenticated caller
/// (frontend contract `nodes/user/@me`, Go controller parity).
fn resolve_user_param(param: &str, caller: Uuid) -> Result<Uuid, ApiError> {
    if param == "@me" {
        return Ok(caller);
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
    input.validate_lengths()?;

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
    let id = crate::types::new_id();
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
    Ok(ApiOk::with_status(StatusCode::OK, json!({"node": created})))
}

/// PUT /nodes/:nodeId — update (write-level access; unlisted+access≥2 open
/// fallback; non-owner edits keep ownership/security fields).
pub async fn update_node(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(node_id): Path<String>,
    Json(input): Json<NodeInput>,
) -> Result<impl IntoResponse, ApiError> {
    input.validate_lengths()?;
    let actor: Actor = auth.into();
    let id = resolve_id(&node_id)?;
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
            let bypass = db_node.access >= 2 && db_node.accessibility == Some(3);
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

    // An omitted parent means "unchanged" (a bare PUT must not silently root
    // the node); a supplied parent is validated below.
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
    // Cycle guard (the frontend `isDescendant` check goes blind under lazy
    // loading, so the server enforces it): the new parent must be neither
    // the node itself nor one of its descendants. Rooting (None) is safe.
    if let Some(pid) = safe_parent
        && pid != db_node.parent_id.unwrap_or_default()
    {
        let subtree = node_repo::subtree_ids(&state.db, id)
            .await
            .map_err(ApiError::from)?;
        if subtree.contains(&pid) {
            return Err(ApiError::BadRequest(
                "cannot move a node into itself or its subtree".into(),
            ));
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

/// DELETE /nodes/:nodeId — delete-level guard; the subtree is moved to the
/// trash (soft delete, spec .work/features/trash.md). Storage objects are NOT
/// touched here: purge/empty is the only place resource files are removed, so
/// a restore always finds its files intact.
pub async fn delete_node(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(node_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let actor: Actor = auth.into();
    let id = resolve_id(&node_id)?;
    let node = guard::require_node_action(&state.db, &actor, id, NodeAction::Delete).await?;
    let (deleted_at, affected) =
        node_repo::soft_delete_subtree(&state.db, node.id, node_repo::now_ms()).await?;
    Ok(ApiOk::ok(json!({
        "deleted_at": deleted_at,
        "nodes": affected,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_size_kept_others_derived() {
        assert_eq!(node_size(4, Some(99), &None, &None), Some(99));
        assert_eq!(
            node_size(3, None, &Some("ab".into()), &Some("cde".into())),
            Some(5)
        );
        assert_eq!(node_size(3, None, &None, &None), Some(0));
    }

    #[test]
    fn length_ceilings_match_varchar() {
        let ok = NodeInput {
            name: "n".repeat(50),
            description: Some("d".repeat(250)),
            tags: Some("t".repeat(250)),
            ..Default::default()
        };
        assert!(ok.validate_lengths().is_ok());
        for (input, field) in [
            (
                NodeInput {
                    name: "n".repeat(51),
                    ..Default::default()
                },
                "Node.Name",
            ),
            (
                NodeInput {
                    name: "n".into(),
                    description: Some("d".repeat(251)),
                    ..Default::default()
                },
                "Node.Description",
            ),
            (
                NodeInput {
                    name: "n".into(),
                    tags: Some("t".repeat(251)),
                    ..Default::default()
                },
                "Node.Tags",
            ),
        ] {
            let err = input.validate_lengths().unwrap_err().to_string();
            assert!(err.contains(field), "missing {field} in: {err}");
            assert!(err.contains("'max'"), "missing 'max' in: {err}");
        }
        // Multi-byte chars count as one (postgres varchar semantics).
        let wide = NodeInput {
            name: "密".repeat(50),
            ..Default::default()
        };
        assert!(wide.validate_lengths().is_ok());
    }

    #[test]
    fn id_resolvers() {
        let id = Uuid::now_v7();
        assert_eq!(resolve_id(&id.to_string()).unwrap(), id);
        assert!(resolve_id("").is_err());
        assert!(resolve_id("nope").is_err());
        assert_eq!(resolve_user_param("@me", id).unwrap(), id);
        assert_eq!(
            resolve_user_param(&id.to_string(), Uuid::nil()).unwrap(),
            id
        );
    }
}
