//! Node read/write queries (sqlx escape hatch — recursive CTE trees, FTS and
//! batched permission loading stay in raw SQL; Go repositories/node.repository.go
//! parity, ported to PostgreSQL dialect).

use sqlx::PgPool;

use crate::models::{NodeRow, NodeSearchRow, PermissionRow, TrashRow};

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
pub async fn list_all(pool: &PgPool, user_id: i64) -> Result<Vec<NodeRow>, sqlx::Error> {
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
pub async fn list_shared(pool: &PgPool, user_id: i64) -> Result<Vec<NodeRow>, sqlx::Error> {
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

/// Permission rows of `user_id` on the given node ids (batched, Go parity).
pub async fn permissions_for_nodes(
    pool: &PgPool,
    user_id: i64,
    node_ids: &[i64],
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

pub async fn get_by_id(pool: &PgPool, node_id: i64) -> Result<Option<NodeRow>, sqlx::Error> {
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
pub async fn get_by_id_any(pool: &PgPool, node_id: i64) -> Result<Option<NodeRow>, sqlx::Error> {
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
pub async fn get_public(pool: &PgPool, node_id: i64) -> Result<Option<NodeRow>, sqlx::Error> {
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
pub async fn public_descendants(pool: &PgPool, node_id: i64) -> Result<Vec<NodeRow>, sqlx::Error> {
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
        SELECT * FROM descendant_nodes ORDER BY role ASC, "order" DESC, name ASC"#,
    )
    .bind(node_id)
    .fetch_all(pool)
    .await
}

/// Total upload size (Go: SUM(size) over the user's nodes).
pub async fn uploads_size(pool: &PgPool, user_id: i64) -> Result<i64, sqlx::Error> {
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
    node_id: i64,
) -> Result<Vec<(i64, i64, Option<serde_json::Value>)>, sqlx::Error> {
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
    user_id: i64,
    node_id: i64,
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
pub struct NodeWrite {
    pub parent_id: Option<i64>,
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

pub async fn insert(
    pool: &PgPool,
    id: i64,
    user_id: i64,
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

pub async fn update(
    pool: &PgPool,
    id: i64,
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

// ---------------------------------------------------------------- trash
// Soft delete (spec .work/features/trash.md): a subtree deleted in one action
// shares a single `deleted_at` stamp, so restore brings the whole batch back.

/// Ids of `node_id` plus every descendant (parent_id chain; node_links do not
/// carry content and are filtered at read time instead of being copied here).
pub async fn subtree_ids(pool: &PgPool, node_id: i64) -> Result<Vec<i64>, sqlx::Error> {
    let rows: Vec<(i64,)> = sqlx::query_as(
        r#"
        WITH RECURSIVE descendants AS (
            SELECT id FROM nodes WHERE id = $1
            UNION ALL
            SELECT n.id FROM nodes n JOIN descendants d ON n.parent_id = d.id
        )
        SELECT id FROM descendants"#,
    )
    .bind(node_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

/// Marks the whole subtree deleted with one shared timestamp. Returns the
/// stamp (used as the batch key for restore) and the affected node count.
pub async fn soft_delete_subtree(
    pool: &PgPool,
    node_id: i64,
    now: i64,
) -> Result<(i64, u64), sqlx::Error> {
    let ids = subtree_ids(pool, node_id).await?;
    if ids.is_empty() {
        return Ok((now, 0));
    }
    let affected = sqlx::query(
        "UPDATE nodes SET deleted_at = $2 WHERE id = ANY($1) AND deleted_at IS NULL",
    )
    .bind(&ids)
    .bind(now)
    .execute(pool)
    .await?
    .rows_affected();
    Ok((now, affected))
}

/// Trash listing: only subtree roots — trashed nodes whose parent is absent or
/// deleted in a different batch. Ordered most recently deleted first.
/// `batch_size` = nodes sharing the same batch stamp under that root, computed
/// in the same query (no N+1).
pub async fn list_trash(pool: &PgPool, user_id: i64) -> Result<Vec<TrashRow>, sqlx::Error> {
    sqlx::query_as::<_, TrashRow>(
        r#"
        WITH RECURSIVE trashed AS (
            SELECT id, parent_id, deleted_at
            FROM nodes WHERE user_id = $1 AND deleted_at IS NOT NULL
        ),
        -- Climb each trashed node to the top of its same-batch chain.
        climb AS (
            SELECT t.id AS node_id, t.id AS cur, t.parent_id, t.deleted_at
            FROM trashed t
            UNION ALL
            SELECT c.node_id, p.id, p.parent_id, c.deleted_at
            FROM climb c
            JOIN nodes p ON p.id = c.parent_id AND p.deleted_at = c.deleted_at
        ),
        batch_root AS (
            SELECT c.node_id, c.cur AS root_id
            FROM climb c
            WHERE NOT EXISTS (
                SELECT 1 FROM nodes p
                WHERE p.id = c.parent_id AND p.deleted_at = c.deleted_at
            )
        ),
        roots AS (
            SELECT id, deleted_at FROM trashed t
            WHERE NOT EXISTS (
                SELECT 1 FROM nodes p
                WHERE p.id = t.parent_id AND p.deleted_at = t.deleted_at
            )
        ),
        batch AS (
            SELECT root_id, COUNT(*)::bigint AS batch_size
            FROM batch_root GROUP BY root_id
        )
        SELECT n.id, n.user_id, n.parent_id, n.name, n.description, n.tags, n.role, n.icon,
               n.size, n.deleted_at, b.batch_size,
               n.created_timestamp, n.updated_timestamp
        FROM roots r
        JOIN nodes n ON n.id = r.id
        LEFT JOIN batch b ON b.root_id = r.id
        ORDER BY n.deleted_at DESC, n.name"#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Restores a trashed subtree. Returns `(restored_count, reparented)` where
/// `reparented` means the node was lifted to root because its parent is still
/// in the trash (spec §3.2 step 3).
pub async fn restore_subtree(
    pool: &PgPool,
    node: &NodeRow,
    deleted_at: i64,
) -> Result<(u64, bool), sqlx::Error> {
    let ids = subtree_ids(pool, node.id).await?;

    // Lift to root when the original parent is still trashed outside this batch.
    let mut reparented = false;
    if let Some(parent_id) = node.parent_id {
        let parent_trashed: Option<(bool,)> = sqlx::query_as(
            "SELECT (deleted_at IS NOT NULL AND deleted_at <> $2) FROM nodes WHERE id = $1",
        )
        .bind(parent_id)
        .bind(deleted_at)
        .fetch_optional(pool)
        .await?;
        if matches!(parent_trashed, Some((true,))) {
            sqlx::query("UPDATE nodes SET parent_id = NULL WHERE id = $1")
                .bind(node.id)
                .execute(pool)
                .await?;
            reparented = true;
        }
    }

    let restored = sqlx::query(
        "UPDATE nodes SET deleted_at = NULL WHERE id = ANY($1) AND deleted_at = $2",
    )
    .bind(&ids)
    .bind(deleted_at)
    .execute(pool)
    .await?
    .rows_affected();
    Ok((restored, reparented))
}

/// Physical delete of a trashed subtree (S3 cleanup happens in the route,
/// before this call). Returns the removed node count.
pub async fn purge_subtree(pool: &PgPool, node_id: i64) -> Result<u64, sqlx::Error> {
    let ids = subtree_ids(pool, node_id).await?;
    if ids.is_empty() {
        return Ok(0);
    }
    let removed = sqlx::query("DELETE FROM nodes WHERE id = ANY($1)")
        .bind(&ids)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(removed)
}

/// Roots of the user's whole trash (used by "empty trash").
pub async fn trash_roots(pool: &PgPool, user_id: i64) -> Result<Vec<i64>, sqlx::Error> {
    let rows: Vec<(i64,)> = sqlx::query_as(
        r#"
        SELECT id FROM nodes n
        WHERE n.user_id = $1
          AND n.deleted_at IS NOT NULL
          AND NOT EXISTS (
              SELECT 1 FROM nodes p
              WHERE p.id = n.parent_id AND p.deleted_at = n.deleted_at
          )"#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
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
    user_id: i64,
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
