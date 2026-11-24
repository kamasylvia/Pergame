//! Flat paged lists for lazy consumers (search results, index pages, home
//! aggregates, pickers): the tree endpoints walk one directory, these answer
//! "which rows, anywhere" without loading the library. All shared-aware
//! (owned UNION granted closure, `list_shared` semantics) with the same sort
//! vocabulary as `tree.rs`.

use sqlx::PgPool;
use uuid::Uuid;

use super::tree::{SortDir, TreeSortKey};

/// Flat list row: metadata columns only (never content).
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct FlatRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub name: String,
    pub description: Option<String>,
    pub tags: Option<String>,
    pub role: i32,
    pub color: Option<i32>,
    pub icon: Option<String>,
    pub theme: Option<String>,
    pub accessibility: Option<i32>,
    pub access: i32,
    pub display: Option<i32>,
    #[serde(rename = "order")]
    pub node_order: Option<i32>,
    pub size: Option<i64>,
    pub metadata: Option<serde_json::Value>,
    pub created_timestamp: i64,
    pub updated_timestamp: i64,
    pub thumbnail: Option<String>,
    #[serde(skip_serializing)]
    pub total: i64,
}

/// Paged flat list, optionally scoped to one role and/or one subtree root
/// (strict descendants, root excluded). Pinned-first narrows to pinned rows.
#[allow(clippy::too_many_arguments)]
pub async fn flat_page(
    pool: &PgPool,
    user_id: Uuid,
    role: Option<i32>,
    root: Option<Uuid>,
    pinned_only: bool,
    offset: i64,
    limit: i64,
    key: TreeSortKey,
    dir: SortDir,
) -> Result<(Vec<FlatRow>, i64), sqlx::Error> {
    let order_by = super::tree::order_clause_on(key, dir, "n");
    // All filters static SQL with NULL-tolerant predicates (every bind is
    // always referenced); only `order_by`/`pin_filter` interpolate, both
    // from fixed vocabularies, never user text.
    let pin_filter = if pinned_only {
        "AND n.\"order\" = -1"
    } else {
        ""
    };
    let sql = format!(
        r#"
        WITH RECURSIVE grant_roots(id) AS (
            SELECT p.node_id FROM permissions p JOIN nodes n ON n.id = p.node_id
            WHERE p.user_id = $1 AND p.permission >= 1 AND n.deleted_at IS NULL
        ),
        shared_ids(id) AS (
            SELECT id FROM grant_roots
            UNION
            SELECT c.id FROM nodes c JOIN shared_ids s ON s.id = c.parent_id
            WHERE c.deleted_at IS NULL
        ),
        visible_ids(id) AS (
            SELECT id FROM nodes WHERE user_id = $1 AND deleted_at IS NULL
            UNION
            SELECT id FROM shared_ids
        ),
        subtree(id) AS (
            SELECT $3::uuid WHERE $3 IS NOT NULL
            UNION ALL
            SELECT c.id FROM nodes c JOIN subtree s ON s.id = c.parent_id
            WHERE c.deleted_at IS NULL
        )
        SELECT {cols}, COUNT(*) OVER() AS total
        FROM nodes n
        JOIN visible_ids v ON v.id = n.id
        WHERE ($2::int IS NULL OR n.role = $2)
          AND ($3::uuid IS NULL OR (n.id <> $3 AND n.id IN (SELECT id FROM subtree)))
          {pin_filter}
        ORDER BY {order_by}
        LIMIT $4 OFFSET $5"#,
        cols = "n.id, n.user_id, n.parent_id, n.name, n.description, n.tags, n.role, n.color, n.icon, n.theme, n.accessibility, n.access, n.display, n.\"order\" AS node_order, n.size, n.metadata, n.created_timestamp, n.updated_timestamp, n.thumbnail",
        pin_filter = pin_filter,
        order_by = order_by,
    );
    let rows: Vec<FlatRow> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .bind(role)
        .bind(root)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
    let total = rows.first().map(|r| r.total).unwrap_or(0);
    Ok((rows, total))
}

/// Counts for stats cards: total live rows plus per-role breakdown, globally
/// or under one subtree root (strict descendants, root excluded).
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct NodeStats {
    pub total: i64,
    pub teams: i64,
    pub workspaces: i64,
    pub folders: i64,
    pub documents: i64,
    pub resources: i64,
}

pub async fn stats(
    pool: &PgPool,
    user_id: Uuid,
    root: Option<Uuid>,
) -> Result<NodeStats, sqlx::Error> {
    let sql = r#"
        WITH RECURSIVE grant_roots(id) AS (
            SELECT p.node_id FROM permissions p JOIN nodes n ON n.id = p.node_id
            WHERE p.user_id = $1 AND p.permission >= 1 AND n.deleted_at IS NULL
        ),
        shared_ids(id) AS (
            SELECT id FROM grant_roots
            UNION
            SELECT c.id FROM nodes c JOIN shared_ids s ON s.id = c.parent_id
            WHERE c.deleted_at IS NULL
        ),
        visible_ids(id) AS (
            SELECT id FROM nodes WHERE user_id = $1 AND deleted_at IS NULL
            UNION
            SELECT id FROM shared_ids
        ),
        subtree(id) AS (
            SELECT $2::uuid WHERE $2 IS NOT NULL
            UNION ALL
            SELECT c.id FROM nodes c JOIN subtree s ON s.id = c.parent_id
            WHERE c.deleted_at IS NULL
        )
        SELECT COUNT(*) AS total,
            COUNT(*) FILTER (WHERE n.role = 0) AS teams,
            COUNT(*) FILTER (WHERE n.role = 1) AS workspaces,
            COUNT(*) FILTER (WHERE n.role = 2) AS folders,
            COUNT(*) FILTER (WHERE n.role = 3) AS documents,
            COUNT(*) FILTER (WHERE n.role = 4) AS resources
        FROM nodes n
        JOIN visible_ids v ON v.id = n.id
        WHERE $2::uuid IS NULL OR (n.id <> $2 AND n.id IN (SELECT id FROM subtree))"#;
    sqlx::query_as::<_, NodeStats>(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .bind(root)
        .fetch_one(pool)
        .await
}

/// Activity histogram: live-node counts per UTC day, newest first.
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct ActivityDay {
    pub day: String,
    pub count: i64,
}

pub async fn activity(
    pool: &PgPool,
    user_id: Uuid,
    limit: i64,
) -> Result<Vec<ActivityDay>, sqlx::Error> {
    sqlx::query_as::<_, ActivityDay>(
        r#"
        WITH RECURSIVE grant_roots(id) AS (
            SELECT p.node_id FROM permissions p JOIN nodes n ON n.id = p.node_id
            WHERE p.user_id = $1 AND p.permission >= 1 AND n.deleted_at IS NULL
        ),
        shared_ids(id) AS (
            SELECT id FROM grant_roots
            UNION
            SELECT c.id FROM nodes c JOIN shared_ids s ON s.id = c.parent_id
            WHERE c.deleted_at IS NULL
        ),
        visible_ids(id) AS (
            SELECT id FROM nodes WHERE user_id = $1 AND deleted_at IS NULL
            UNION
            SELECT id FROM shared_ids
        )
        SELECT to_char(to_timestamp(n.updated_timestamp / 1000), 'YYYY-MM-DD') AS day, COUNT(*) AS count
        FROM nodes n
        JOIN visible_ids v ON v.id = n.id
        GROUP BY day
        ORDER BY day DESC
        LIMIT $2"#,
    )
    .bind(user_id)
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// Distinct raw tag strings (the frontend parses/unions, as before).
pub async fn distinct_tags(pool: &PgPool, user_id: Uuid) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        r#"
        WITH RECURSIVE grant_roots(id) AS (
            SELECT p.node_id FROM permissions p JOIN nodes n ON n.id = p.node_id
            WHERE p.user_id = $1 AND p.permission >= 1 AND n.deleted_at IS NULL
        ),
        shared_ids(id) AS (
            SELECT id FROM grant_roots
            UNION
            SELECT c.id FROM nodes c JOIN shared_ids s ON s.id = c.parent_id
            WHERE c.deleted_at IS NULL
        ),
        visible_ids(id) AS (
            SELECT id FROM nodes WHERE user_id = $1 AND deleted_at IS NULL
            UNION
            SELECT id FROM shared_ids
        )
        SELECT DISTINCT n.tags
        FROM nodes n
        JOIN visible_ids v ON v.id = n.id
        WHERE n.tags IS NOT NULL AND n.tags <> ''
        ORDER BY 1"#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Adjacent siblings of one node under an ordering (prev/next navigation):
/// window functions over the visible, ordered sibling set — no full load.
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct NeighborRow {
    pub id: Uuid,
    pub name: String,
    pub role: i32,
}

pub async fn neighbors(
    pool: &PgPool,
    user_id: Uuid,
    node_id: Uuid,
    key: TreeSortKey,
    dir: SortDir,
) -> Result<(Option<NeighborRow>, Option<NeighborRow>), sqlx::Error> {
    let order_by = super::tree::order_clause(key, dir);
    // Audited: static ORDER BY fragment; all values bound.
    let sql = format!(
        r#"
        WITH RECURSIVE chain(id, parent_id, user_id, depth) AS (
            SELECT id, parent_id, user_id, 0 FROM nodes WHERE id = (SELECT parent_id FROM nodes WHERE id = $1)
            UNION ALL
            SELECT n.id, n.parent_id, n.user_id, c.depth + 1
            FROM nodes n JOIN chain c ON c.parent_id = n.id
            WHERE c.depth < 100
        ),
        chain_open AS (
            SELECT 1 AS ok
            WHERE EXISTS (SELECT 1 FROM permissions p JOIN chain c ON c.id = p.node_id WHERE p.user_id = $2 AND p.permission >= 1)
               OR EXISTS (SELECT 1 FROM chain WHERE user_id = $2)
        ),
        sibs AS (
            SELECT id, name, role, ROW_NUMBER() OVER (ORDER BY {order_by}) AS rn FROM (
                SELECT n.id, n.name, n.role, n."order" AS node_order, n.created_timestamp, n.updated_timestamp
                FROM nodes n
                WHERE n.parent_id IS NOT DISTINCT FROM (SELECT parent_id FROM nodes WHERE id = $1)
                  AND n.deleted_at IS NULL
                  AND (n.user_id = $2
                    OR EXISTS (SELECT 1 FROM permissions p WHERE p.node_id = n.id AND p.user_id = $2 AND p.permission >= 1)
                    OR EXISTS (SELECT 1 FROM chain_open))
            ) s
        )
        SELECT id, name, role, -1 AS side FROM sibs WHERE rn = (SELECT rn FROM sibs WHERE id = $1) - 1
        UNION ALL
        SELECT id, name, role, 1 AS side FROM sibs WHERE rn = (SELECT rn FROM sibs WHERE id = $1) + 1"#,
        order_by = order_by,
    );
    let rows: Vec<(Uuid, String, i32, i32)> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(node_id)
        .bind(user_id)
        .fetch_all(pool)
        .await?;
    let mut prev = None;
    let mut next = None;
    for (id, name, role, side) in rows {
        let row = NeighborRow { id, name, role };
        if side < 0 {
            prev = Some(row);
        } else {
            next = Some(row);
        }
    }
    Ok((prev, next))
}

/// Substring search over name/description/tags (ILIKE, any length — covers
/// the single-char queries FTS refuses). Shared-aware; newest first.
pub async fn meta_search(
    pool: &PgPool,
    user_id: Uuid,
    query: &str,
    role: Option<i32>,
    limit: i64,
) -> Result<Vec<FlatRow>, sqlx::Error> {
    // LIKE metacharacters are escaped so the query stays literal.
    let pattern = format!(
        "%{}%",
        query
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    );
    let sql = r#"
        WITH RECURSIVE grant_roots(id) AS (
            SELECT p.node_id FROM permissions p JOIN nodes n ON n.id = p.node_id
            WHERE p.user_id = $1 AND p.permission >= 1 AND n.deleted_at IS NULL
        ),
        shared_ids(id) AS (
            SELECT id FROM grant_roots
            UNION
            SELECT c.id FROM nodes c JOIN shared_ids s ON s.id = c.parent_id
            WHERE c.deleted_at IS NULL
        ),
        visible_ids(id) AS (
            SELECT id FROM nodes WHERE user_id = $1 AND deleted_at IS NULL
            UNION
            SELECT id FROM shared_ids
        )
        SELECT n.id, n.user_id, n.parent_id, n.name, n.description, n.tags, n.role, n.color, n.icon, n.theme, n.accessibility, n.access, n.display, n."order" AS node_order, n.size, n.metadata, n.created_timestamp, n.updated_timestamp, n.thumbnail, 0::bigint AS total
        FROM nodes n
        JOIN visible_ids v ON v.id = n.id
        WHERE (n.name ILIKE $2 ESCAPE '\' OR n.description ILIKE $2 ESCAPE '\' OR n.tags ILIKE $2 ESCAPE '\')
          AND ($3::int IS NULL OR n.role = $3)
        ORDER BY n.updated_timestamp DESC, n.id ASC
        LIMIT $4"#;
    sqlx::query_as::<_, FlatRow>(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .bind(pattern)
        .bind(role)
        .bind(limit)
        .fetch_all(pool)
        .await
}
