//! Paged tree reads for lazy directory loading (SiYuan `listDocTreeRange`
//! parity): one directory page at a time instead of `list_all`.
//!
//! Order mirrors the frontend `sortNodes` comparator exactly (role DESC,
//! `COALESCE(node_order,0)` ASC, key ASC/DESC) plus an `id` tiebreak the client
//! sort lacks — without a total order, page boundaries drift. Note the DB
//! collation and JS `localeCompare` can disagree on CJK names; the server
//! order wins and the client must display pages as-is, never re-sort.
//!
//! Visibility = owned, directly granted (permission >= READ), or an open
//! chain (owned/granted ancestor, Go `HasPermission` parity). `has_children`
//! is deliberately UNFILTERED (canonical children or link positions, live
//! rows only): a missing chevron would strand reachable data, while an extra
//! chevron merely expands to an empty list.

use sqlx::PgPool;
use uuid::Uuid;

/// One directory row: the `list_all` column set (no content columns) plus
/// the tree extras. `total` repeats per row (`COUNT(*) OVER()`) and is
/// stripped on the wire; the handler hoists it to the envelope.
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct ChildRow {
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
    pub link_id: Option<Uuid>,
    pub has_children: bool,
    #[serde(skip_serializing)]
    pub total: i64,
}

/// Sort key matching the `sortKey` preference (`name`/`created`/`modified`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeSortKey {
    Name,
    Created,
    Modified,
}

/// Sort direction matching the `sortOrder` preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDir {
    Asc,
    Desc,
}

impl TreeSortKey {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "name" => Some(Self::Name),
            "created" => Some(Self::Created),
            "modified" => Some(Self::Modified),
            _ => None,
        }
    }
}

impl SortDir {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "asc" | "ascending" => Some(Self::Asc),
            "desc" | "descending" => Some(Self::Desc),
            _ => None,
        }
    }
}

/// ORDER BY fragment over the CTE output columns (note `node_order`, not
/// `"order"` — the raw column name is invisible outside the CTE).
/// Static strings only — sort params never interpolate.
pub fn order_clause(key: TreeSortKey, dir: SortDir) -> &'static str {
    match (key, dir) {
        (TreeSortKey::Name, SortDir::Asc) => {
            "role DESC, COALESCE(node_order, 0) ASC, name ASC, id ASC"
        }
        (TreeSortKey::Name, SortDir::Desc) => {
            "role DESC, COALESCE(node_order, 0) ASC, name DESC, id ASC"
        }
        (TreeSortKey::Created, SortDir::Asc) => {
            "role DESC, COALESCE(node_order, 0) ASC, created_timestamp ASC, id ASC"
        }
        (TreeSortKey::Created, SortDir::Desc) => {
            "role DESC, COALESCE(node_order, 0) ASC, created_timestamp DESC, id ASC"
        }
        (TreeSortKey::Modified, SortDir::Asc) => {
            "role DESC, COALESCE(node_order, 0) ASC, updated_timestamp ASC, id ASC"
        }
        (TreeSortKey::Modified, SortDir::Desc) => {
            "role DESC, COALESCE(node_order, 0) ASC, updated_timestamp DESC, id ASC"
        }
    }
}

/// Column list mirroring `list_all` (no content columns), qualified per
/// UNION branch. `$a` is the branch table alias.
macro_rules! child_cols {
    ($a:literal) => {
        concat!(
            $a,
            ".id, ",
            $a,
            ".user_id, ",
            $a,
            ".parent_id, ",
            $a,
            ".name, ",
            $a,
            ".description, ",
            $a,
            ".tags, ",
            $a,
            ".role, ",
            $a,
            ".color, ",
            $a,
            ".icon, ",
            $a,
            ".theme, ",
            $a,
            ".accessibility, ",
            $a,
            ".access, ",
            $a,
            ".display, ",
            $a,
            ".\"order\" AS node_order, ",
            $a,
            ".size, ",
            $a,
            ".metadata, ",
            $a,
            ".created_timestamp, ",
            $a,
            ".updated_timestamp, CASE WHEN ",
            $a,
            ".role IN (1, 2) THEN ",
            $a,
            ".thumbnail ELSE NULL END AS thumbnail"
        )
    };
}

/// One page of a directory: canonical children visible to the caller plus
/// their clone positions under it (a node already canonical here never
/// double-appears as a link, TreeBuilder parity).
pub async fn children_page(
    pool: &PgPool,
    user_id: Uuid,
    parent_id: Uuid,
    offset: i64,
    limit: i64,
    key: TreeSortKey,
    dir: SortDir,
) -> Result<(Vec<ChildRow>, i64), sqlx::Error> {
    let order_by = order_clause(key, dir);
    let sql = format!(
        r#"
        WITH RECURSIVE chain(id, parent_id, user_id, depth) AS (
            SELECT id, parent_id, user_id, 0 FROM nodes WHERE id = $1
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
        kids AS (
            SELECT {}, NULL::uuid AS link_id FROM nodes c
            WHERE c.parent_id = $1 AND c.deleted_at IS NULL
              AND (c.user_id = $2
                OR EXISTS (SELECT 1 FROM permissions p WHERE p.node_id = c.id AND p.user_id = $2 AND p.permission >= 1)
                OR EXISTS (SELECT 1 FROM chain_open))
            UNION ALL
            SELECT {}, nl.id AS link_id FROM nodes c
            JOIN node_links nl ON nl.node_id = c.id AND nl.parent_id = $1
            WHERE c.deleted_at IS NULL AND c.parent_id IS DISTINCT FROM $1 AND c.user_id = $2
        )
        SELECT *, COUNT(*) OVER() AS total,
            (EXISTS (SELECT 1 FROM nodes k WHERE k.parent_id = kids.id AND k.deleted_at IS NULL)
             OR EXISTS (SELECT 1 FROM node_links nl WHERE nl.parent_id = kids.id)) AS has_children
        FROM kids
        ORDER BY {order_by}
        LIMIT $3 OFFSET $4"#,
        child_cols!("c"),
        child_cols!("c"),
        order_by = order_by,
    );
    // Audited: only `order_by` interpolates (six static fragments, never user
    // text); all values are bound — same pattern as the DQL planner.
    let rows: Vec<ChildRow> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(parent_id)
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
    let total = rows.first().map(|r| r.total).unwrap_or(0);
    Ok((rows, total))
}

/// Top-level rows: owned workspaces/teams and parentless nodes, plus direct
/// grants (the frontend nests a grant under its parent when loaded and roots
/// it otherwise — same visible set the eager tree derived from `fetchShared`
/// plus the root filter).
pub async fn roots_page(
    pool: &PgPool,
    user_id: Uuid,
    offset: i64,
    limit: i64,
    key: TreeSortKey,
    dir: SortDir,
) -> Result<(Vec<ChildRow>, i64), sqlx::Error> {
    let order_by = order_clause(key, dir);
    let sql = format!(
        r#"
        WITH roots AS (
            SELECT {}, NULL::uuid AS link_id
            FROM nodes n
            WHERE n.user_id = $1 AND n.deleted_at IS NULL AND (n.role <= 1 OR n.parent_id IS NULL)
            UNION ALL
            SELECT {}, NULL::uuid AS link_id
            FROM nodes n
            JOIN permissions p ON p.node_id = n.id
            WHERE p.user_id = $1 AND p.permission >= 1 AND n.deleted_at IS NULL
              AND NOT (n.user_id = $1 AND (n.role <= 1 OR n.parent_id IS NULL))
        )
        SELECT *, COUNT(*) OVER() AS total,
            (EXISTS (SELECT 1 FROM nodes k WHERE k.parent_id = roots.id AND k.deleted_at IS NULL)
             OR EXISTS (SELECT 1 FROM node_links nl WHERE nl.parent_id = roots.id)) AS has_children
        FROM roots
        ORDER BY {order_by}
        LIMIT $2 OFFSET $3"#,
        child_cols!("n"),
        child_cols!("n"),
        order_by = order_by,
    );
    // Audited: see `children_page` — static ORDER BY fragment, bound values.
    let rows: Vec<ChildRow> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
    let total = rows.first().map(|r| r.total).unwrap_or(0);
    Ok((rows, total))
}

/// Ancestor chain leaf-first (`[self … root]`, `getAncestors` parity) for
/// breadcrumbs, reveal and permission walks. Depth-capped against corrupt
/// cycles; callers must establish read access on the node first.
pub async fn ancestors(
    pool: &PgPool,
    node_id: Uuid,
) -> Result<Vec<crate::models::NodeRow>, sqlx::Error> {
    sqlx::query_as::<_, crate::models::NodeRow>(
        r#"
        WITH RECURSIVE up(id, user_id, parent_id, name, description, tags, role, color, icon, theme, accessibility, access, display, node_order, size, metadata, created_timestamp, updated_timestamp, thumbnail, depth) AS (
            SELECT id, user_id, parent_id, name, description, tags, role, color, icon, theme, accessibility, access, display, "order", size, metadata, created_timestamp, updated_timestamp, CASE WHEN role IN (1, 2) THEN thumbnail ELSE NULL END, 0
            FROM nodes WHERE id = $1 AND deleted_at IS NULL
            UNION ALL
            SELECT n.id, n.user_id, n.parent_id, n.name, n.description, n.tags, n.role, n.color, n.icon, n.theme, n.accessibility, n.access, n.display, n."order", n.size, n.metadata, n.created_timestamp, n.updated_timestamp, CASE WHEN n.role IN (1, 2) THEN n.thumbnail ELSE NULL END, u.depth + 1
            FROM nodes n JOIN up u ON u.parent_id = n.id
            WHERE n.deleted_at IS NULL AND u.depth < 100
        )
        SELECT id, user_id, parent_id, name, description, tags, role, color, icon, theme, accessibility, access, display, node_order, size, metadata, created_timestamp, updated_timestamp, thumbnail FROM up ORDER BY depth ASC"#,
    )
    .bind(node_id)
    .fetch_all(pool)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_params_parse() {
        assert_eq!(TreeSortKey::parse("name"), Some(TreeSortKey::Name));
        assert_eq!(TreeSortKey::parse("created"), Some(TreeSortKey::Created));
        assert_eq!(TreeSortKey::parse("modified"), Some(TreeSortKey::Modified));
        assert_eq!(TreeSortKey::parse("size"), None);
        assert_eq!(SortDir::parse("asc"), Some(SortDir::Asc));
        assert_eq!(SortDir::parse("ascending"), Some(SortDir::Asc));
        assert_eq!(SortDir::parse("desc"), Some(SortDir::Desc));
        assert_eq!(SortDir::parse("descending"), Some(SortDir::Desc));
        assert_eq!(SortDir::parse("up"), None);
    }

    #[test]
    fn child_cols_cover_row() {
        // The CTE output must name every ChildRow column (a dropped `id`
        // once broke all three queries with "column roots.id does not exist").
        let cols = child_cols!("n");
        for c in ["n.id,", "n.user_id,", "AS node_order", "AS thumbnail"] {
            assert!(cols.contains(c), "missing {c} in: {cols}");
        }
    }

    #[test]
    fn order_clause_mirrors_frontend_sort() {
        // role DESC, COALESCE(order,0) ASC are unconditional (frontend flips
        // only the key comparison); id ASC always terminates for page stability.
        for key in [
            TreeSortKey::Name,
            TreeSortKey::Created,
            TreeSortKey::Modified,
        ] {
            for dir in [SortDir::Asc, SortDir::Desc] {
                let clause = order_clause(key, dir);
                assert!(
                    clause.starts_with("role DESC, COALESCE(node_order, 0) ASC, "),
                    "{clause}"
                );
                assert!(clause.ends_with(", id ASC"), "{clause}");
            }
        }
        assert!(order_clause(TreeSortKey::Name, SortDir::Asc).contains("name ASC"));
        assert!(order_clause(TreeSortKey::Name, SortDir::Desc).contains("name DESC"));
        assert!(order_clause(TreeSortKey::Created, SortDir::Asc).contains("created_timestamp ASC"));
        assert!(
            order_clause(TreeSortKey::Modified, SortDir::Desc).contains("updated_timestamp DESC")
        );
    }
}
