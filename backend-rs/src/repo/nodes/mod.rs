//! Node read/write queries (sqlx escape hatch — recursive CTE trees, FTS and
//! batched permission loading stay in raw SQL; Go repositories/node.repository.go
//! parity, ported to PostgreSQL dialect).

pub mod trash;

pub use self::trash::{
    list_trash, purge_subtree, restore_subtree, soft_delete_subtree, subtree_ids, trash_roots,
};

use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{NodeRow, NodeSearchRow, PermissionRow};

/// Wall-clock unix millis (node timestamps).
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// All nodes owned by the user plus children of owned nodes (recursive),
/// folder-role thumbnails only; ordering mirrors the Go query (role asc,
/// pinned first, name). Go's `'order' DESC` literal was a no-op string sort
/// key; the real pinned-first behavior comes from the frontend.
pub async fn list_all(pool: &PgPool, user_id: Uuid) -> Result<Vec<NodeRow>, sqlx::Error> {
    sqlx::query_as::<_, NodeRow>(
        r#"
        WITH RECURSIVE user_nodes AS (
            SELECT n.id, n.user_id, n.parent_id, n.name, n.description, n.tags, n.role, n.color,
                   n.icon, n.theme, n.accessibility, n.access, n.display, n."order" AS node_order, n.size,
                   n.metadata, n.created_timestamp, n.updated_timestamp,
                   CASE WHEN n.role IN (1, 2) THEN n.thumbnail ELSE NULL END AS thumbnail
            FROM nodes n
            WHERE n.user_id = $1 AND n.deleted_at IS NULL
            UNION
            SELECT c.id, c.user_id, c.parent_id, c.name, c.description, c.tags, c.role, c.color,
                   c.icon, c.theme, c.accessibility, c.access, c.display, c."order" AS node_order, c.size,
                   c.metadata, c.created_timestamp, c.updated_timestamp,
                   CASE WHEN c.role IN (1, 2) THEN c.thumbnail ELSE NULL END AS thumbnail
            FROM nodes c
            JOIN user_nodes un
              ON un.id = c.parent_id
              OR EXISTS (SELECT 1 FROM node_links nl
                         WHERE nl.node_id = c.id AND nl.parent_id = un.id)
            WHERE c.deleted_at IS NULL
        )
        SELECT * FROM user_nodes ORDER BY role, name"#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Nodes shared with the user via the permissions table (with descendants).
pub async fn list_shared(pool: &PgPool, user_id: Uuid) -> Result<Vec<NodeRow>, sqlx::Error> {
    sqlx::query_as::<_, NodeRow>(
        r#"
        WITH RECURSIVE shared_nodes AS (
            SELECT n.id, n.user_id, n.parent_id, n.name, n.description, n.tags, n.role, n.color,
                   n.icon, n.theme, n.accessibility, n.access, n.display, n."order" AS node_order, n.size,
                   n.metadata, n.created_timestamp, n.updated_timestamp
            FROM nodes n
            JOIN permissions p ON p.node_id = n.id
            WHERE p.user_id = $1 AND n.deleted_at IS NULL
            UNION
            SELECT c.id, c.user_id, c.parent_id, c.name, c.description, c.tags, c.role, c.color,
                   c.icon, c.theme, c.accessibility, c.access, c.display, c."order" AS node_order, c.size,
                   c.metadata, c.created_timestamp, c.updated_timestamp
            FROM nodes c
            JOIN shared_nodes an
              ON an.id = c.parent_id
              OR EXISTS (SELECT 1 FROM node_links nl
                         WHERE nl.node_id = c.id AND nl.parent_id = an.id)
            WHERE c.deleted_at IS NULL
        )
        SELECT * FROM shared_nodes"#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Object keys still referenced by this user's resource nodes, trashed ones
/// included: a file in the trash must survive until the trash is purged, so
/// orphan detection can never treat it as garbage.
pub async fn referenced_resource_paths(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_as::<_, (String,)>(
        "SELECT metadata->>'transformed_path' FROM nodes
         WHERE user_id = $1 AND role = 4 AND metadata ? 'transformed_path'",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map(|rows| rows.into_iter().map(|(p,)| p).collect())
}

/// Permission rows of `user_id` on the given node ids (batched, Go parity).
pub async fn permissions_for_nodes(
    pool: &PgPool,
    user_id: Uuid,
    node_ids: &[Uuid],
) -> Result<Vec<PermissionRow>, sqlx::Error> {
    if node_ids.is_empty() {
        return Ok(vec![]);
    }
    sqlx::query_as::<_, PermissionRow>(
        r#"SELECT id, node_id, user_id, permission, created_timestamp
           FROM permissions
           WHERE user_id = $1 AND node_id = ANY($2)"#,
    )
    .bind(user_id)
    .bind(node_ids)
    .fetch_all(pool)
    .await
}

/// One live node by id (soft-deleted excluded).
pub async fn get_by_id(pool: &PgPool, node_id: Uuid) -> Result<Option<NodeRow>, sqlx::Error> {
    // Column list spelled out (sqlx audit lint; same as NODE_FULL_COLS).
    sqlx::query_as::<_, NodeRow>(
        "SELECT id, user_id, parent_id, name, description, tags, role, color, icon, \
         thumbnail, theme, accessibility, access, display, \"order\" AS node_order, content, \
         content_compiled, size, metadata, created_timestamp, updated_timestamp \
         FROM nodes WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(node_id)
    .fetch_optional(pool)
    .await
}

/// Same as `get_by_id` but also returns trashed rows (trash endpoints only).
pub async fn get_by_id_any(pool: &PgPool, node_id: Uuid) -> Result<Option<NodeRow>, sqlx::Error> {
    sqlx::query_as::<_, NodeRow>(
        "SELECT id, user_id, parent_id, name, description, tags, role, color, icon, \
         thumbnail, theme, accessibility, access, display, \"order\" AS node_order, content, \
         content_compiled, size, metadata, created_timestamp, updated_timestamp \
         FROM nodes WHERE id = $1",
    )
    .bind(node_id)
    .fetch_optional(pool)
    .await
}

/// The node if it or any ancestor is public (accessibility = 3).
pub async fn get_public(pool: &PgPool, node_id: Uuid) -> Result<Option<NodeRow>, sqlx::Error> {
    sqlx::query_as::<_, NodeRow>(
        r#"
        WITH RECURSIVE ancestors AS (
            SELECT id, parent_id, accessibility FROM nodes WHERE id = $1 AND deleted_at IS NULL
            UNION ALL
            SELECT n.id, n.parent_id, n.accessibility
            FROM nodes n JOIN ancestors a ON n.id = a.parent_id
            WHERE n.deleted_at IS NULL
        )
        SELECT id, user_id, parent_id, name, description, tags, role, color, icon, thumbnail, theme,
               accessibility, access, display, "order" AS node_order, content, content_compiled, size, metadata,
               created_timestamp, updated_timestamp
        FROM nodes
        WHERE id = $1 AND deleted_at IS NULL
          AND EXISTS (SELECT 1 FROM ancestors WHERE accessibility = 3)"#,
    )
    .bind(node_id)
    .fetch_optional(pool)
    .await
}

/// All descendants of a folder-ish public node (roles 1/2/3 inherit access).
pub async fn public_descendants(pool: &PgPool, node_id: Uuid) -> Result<Vec<NodeRow>, sqlx::Error> {
    sqlx::query_as::<_, NodeRow>(
        r#"
        WITH RECURSIVE descendant_nodes AS (
            SELECT id, user_id, parent_id, name, description, tags, role, color, icon, thumbnail,
                   theme, accessibility, access, display, "order" AS node_order, size, metadata,
                   created_timestamp, updated_timestamp
            FROM nodes
            WHERE parent_id = $1 AND role IN (1, 2, 3) AND deleted_at IS NULL
            UNION
            SELECT n.id, n.user_id, n.parent_id, n.name, n.description, n.tags, n.role, n.color,
                   n.icon, n.thumbnail, n.theme, n.accessibility, n.access, n.display, n."order" AS node_order,
                   n.size, n.metadata, n.created_timestamp, n.updated_timestamp
            FROM nodes n
            JOIN descendant_nodes d
              ON d.id = n.parent_id
              OR EXISTS (SELECT 1 FROM node_links nl
                         WHERE nl.node_id = n.id AND nl.parent_id = d.id)
            WHERE n.role IN (1, 2, 3) AND n.deleted_at IS NULL
        )
        SELECT * FROM descendant_nodes ORDER BY role ASC, node_order DESC NULLS LAST, name ASC"#,
    )
    .bind(node_id)
    .fetch_all(pool)
    .await
}

/// Total upload size (Go: SUM(size) over the user's nodes).
pub async fn uploads_size(pool: &PgPool, user_id: Uuid) -> Result<i64, sqlx::Error> {
    let row: (i64,) =
        sqlx::query_as("SELECT COALESCE(SUM(size), 0)::bigint FROM nodes WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(pool)
            .await?;
    Ok(row.0)
}

/// Descendant resource rows (role=4) for storage cleanup before deletion.
pub async fn descendant_resources(
    pool: &PgPool,
    node_id: Uuid,
) -> Result<Vec<(Uuid, Uuid, Option<serde_json::Value>)>, sqlx::Error> {
    sqlx::query_as(
        r#"
        WITH RECURSIVE descendants AS (
            SELECT id, user_id, role, metadata FROM nodes WHERE id = $1
            UNION ALL
            SELECT n.id, n.user_id, n.role, n.metadata
            FROM nodes n JOIN descendants d ON n.parent_id = d.id
        )
        SELECT id, user_id, metadata FROM descendants WHERE role = 4"#,
    )
    .bind(node_id)
    .fetch_all(pool)
    .await
}

/// Ancestor-chain permission: MAX(permission) over permissions on the node or
/// any ancestor; falls back to PermOwner(4) when the user owns an ancestor
/// (Go HasPermission parity).
pub async fn has_permission(
    pool: &PgPool,
    user_id: Uuid,
    node_id: Uuid,
    required: i32,
) -> (bool, i32) {
    let max_perm: Option<Option<i32>> = sqlx::query_scalar(
        r#"
        WITH RECURSIVE ancestors AS (
            SELECT id, parent_id FROM nodes WHERE id = $1
            UNION ALL
            SELECT n.id, n.parent_id FROM nodes n
            INNER JOIN ancestors a ON a.parent_id = n.id
        )
        SELECT MAX(p.permission)
        FROM permissions p
        INNER JOIN ancestors an ON an.id = p.node_id
        WHERE p.user_id = $2"#,
    )
    .bind(node_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .ok();

    if let Some(Some(level)) = max_perm
        && level >= required
    {
        return (true, level);
    }

    // Ancestor-owner fallback.
    let owns: Option<i32> = sqlx::query_scalar(
        r#"
        WITH RECURSIVE ancestors AS (
            SELECT id, parent_id, user_id FROM nodes WHERE id = $1
            UNION ALL
            SELECT n.id, n.parent_id, n.user_id FROM nodes n
            INNER JOIN ancestors a ON a.parent_id = n.id
        )
        SELECT 1 FROM ancestors WHERE user_id = $2 LIMIT 1"#,
    )
    .bind(node_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    if owns == Some(1) {
        return (true, 4);
    }
    (false, 0)
}

#[derive(Debug, Clone, Default)]
/// Writable node fields for insert/update.
pub struct NodeWrite {
    pub parent_id: Option<Uuid>,
    pub name: String,
    pub description: Option<String>,
    pub tags: Option<String>,
    pub role: i32,
    pub color: Option<i32>,
    pub icon: Option<String>,
    pub thumbnail: Option<String>,
    pub theme: Option<String>,
    pub accessibility: Option<i32>,
    pub access: i32,
    pub display: Option<i32>,
    pub node_order: Option<i32>,
    pub content: Option<String>,
    pub content_compiled: Option<String>,
    pub size: Option<i64>,
    pub metadata: Option<serde_json::Value>,
}

/// Insert a node row.
pub async fn insert(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
    n: &NodeWrite,
    now: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO nodes (id, user_id, parent_id, name, description, tags, role, color, icon,
            thumbnail, theme, accessibility, access, display, "order", content, content_compiled,
            size, metadata, created_timestamp, updated_timestamp)
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21)"#,
    )
    .bind(id)
    .bind(user_id)
    .bind(n.parent_id)
    .bind(&n.name)
    .bind(&n.description)
    .bind(&n.tags)
    .bind(n.role)
    .bind(n.color)
    .bind(&n.icon)
    .bind(&n.thumbnail)
    .bind(&n.theme)
    .bind(n.accessibility)
    .bind(n.access)
    .bind(n.display)
    .bind(n.node_order)
    .bind(&n.content)
    .bind(&n.content_compiled)
    .bind(n.size)
    .bind(&n.metadata)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Update a node row (all writable fields).
pub async fn update(
    pool: &PgPool,
    id: Uuid,
    n: &NodeWrite,
    updated: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"UPDATE nodes
           SET parent_id = $1, name = $2, description = $3, tags = $4, role = $5, color = $6,
               icon = $7, thumbnail = $8, theme = $9, accessibility = $10, access = $11,
               display = $12, "order" = $13, content = $14, content_compiled = $15, size = $16,
               metadata = $17, updated_timestamp = $18
           WHERE id = $19"#,
    )
    .bind(n.parent_id)
    .bind(&n.name)
    .bind(&n.description)
    .bind(&n.tags)
    .bind(n.role)
    .bind(n.color)
    .bind(&n.icon)
    .bind(&n.thumbnail)
    .bind(&n.theme)
    .bind(n.accessibility)
    .bind(n.access)
    .bind(n.display)
    .bind(n.node_order)
    .bind(&n.content)
    .bind(&n.content_compiled)
    .bind(n.size)
    .bind(&n.metadata)
    .bind(updated)
    .bind(id)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Builds a 'simple'-dialect tsquery string of OR-ed tokens from the raw query
/// (closest pg equivalent of MySQL NATURAL LANGUAGE MODE's any-word match).
pub fn to_or_tsquery(query: &str) -> String {
    query
        .split_whitespace()
        .map(|t| {
            let cleaned: String = t.chars().filter(|c| c.is_alphanumeric()).collect();
            cleaned
        })
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join(" | ")
}

/// Full-text search: content path (role=3 docs) or metadata path. Ranking via
/// ts_rank; snippet via ts_headline (behavioral upgrade over the Go
/// LOCATE/SUBSTRING truncation, documented in the research report §3).
pub async fn search(
    pool: &PgPool,
    user_id: Uuid,
    query: &str,
    include_content: bool,
    limit: i32,
) -> Result<Vec<NodeSearchRow>, sqlx::Error> {
    let limit = limit.clamp(1, 100);
    let clean = query.trim().to_string();
    if clean.chars().count() < 2 {
        return Ok(vec![]);
    }
    let tsq = to_or_tsquery(&clean);
    if tsq.is_empty() {
        return Ok(vec![]);
    }

    if include_content {
        sqlx::query_as::<_, NodeSearchRow>(
            r#"
            SELECT n.id, n.user_id, n.parent_id, n.name, n.description, n.tags, n.role,
                   n.accessibility, n.icon,
                   ts_rank(n.content_tsv, to_tsquery('simple', $2))::float8 AS relevance,
                   ts_headline('simple', n.content, to_tsquery('simple', $2),
                               'MaxWords=35, MinWords=15, MaxFragments=1') AS content_snippet,
                   n.created_timestamp, n.updated_timestamp
            FROM nodes n
            WHERE n.user_id = $1 AND n.role = 3 AND n.deleted_at IS NULL
              AND n.content_tsv @@ to_tsquery('simple', $2)
            ORDER BY relevance DESC
            LIMIT $3"#,
        )
        .bind(user_id)
        .bind(&tsq)
        .bind(limit)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as::<_, NodeSearchRow>(
            r#"
            SELECT n.id, n.user_id, n.parent_id, n.name, n.description, n.tags, n.role,
                   n.accessibility, n.icon,
                   ts_rank(n.search_tsv, to_tsquery('simple', $2))::float8 AS relevance,
                   NULL::text AS content_snippet,
                   n.created_timestamp, n.updated_timestamp
            FROM nodes n
            WHERE n.user_id = $1 AND n.role = 3 AND n.deleted_at IS NULL
              AND n.search_tsv @@ to_tsquery('simple', $2)
            ORDER BY relevance DESC
            LIMIT $3"#,
        )
        .bind(user_id)
        .bind(&tsq)
        .bind(limit)
        .fetch_all(pool)
        .await
    }
}

/// Resolve a folder path ("Projects" or "Projects/2024") to its node id,
/// walking name segments down from the user's roots. `None` = not found.
pub async fn resolve_folder_path(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    path: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    let segs: Vec<&str> = path.split('/').map(str::trim).filter(|s| !s.is_empty()).collect();
    if segs.is_empty() {
        return Ok(None);
    }
    let mut parent: Option<Uuid> = None;
    for seg in segs {
        let id: Option<(Uuid,)> = match parent {
            None => {
                sqlx::query_as(
                    "SELECT id FROM nodes WHERE user_id = $1 AND parent_id IS NULL AND name = $2 AND role IN (1,2) AND deleted_at IS NULL",
                )
                .bind(user_id)
                .bind(seg)
                .fetch_optional(pool)
                .await?
            }
            Some(p) => {
                sqlx::query_as(
                    "SELECT id FROM nodes WHERE user_id = $1 AND parent_id = $2 AND name = $3 AND role = 2 AND deleted_at IS NULL",
                )
                .bind(user_id)
                .bind(p)
                .bind(seg)
                .fetch_optional(pool)
                .await?
            }
        };
        match id {
            Some((id,)) => parent = Some(id),
            None => return Ok(None),
        }
    }
    Ok(parent)
}

/// Execute a planned DQL statement (dynamic binding count → manual binds).
/// SAFETY: `planned.sql` is planner-generated (field/function whitelist,
/// all values bound as parameters); audited, hence `AssertSqlSafe`.
pub async fn dql_query(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    planned: &crate::dql::Planned,
) -> Result<Vec<NodeRow>, sqlx::Error> {
    let mut q =
        sqlx::query_as::<_, NodeRow>(sqlx::AssertSqlSafe(planned.sql.clone())).bind(user_id);
    for b in &planned.bindings {
        q = match b {
            crate::dql::BoundValue::Str(s) => q.bind(s),
            crate::dql::BoundValue::I64(n) => q.bind(n),
            crate::dql::BoundValue::F64(n) => q.bind(n),
        };
    }
    q.fetch_all(pool).await
}
