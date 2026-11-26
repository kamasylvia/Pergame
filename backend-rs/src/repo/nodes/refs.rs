//! Internal-link index (`node_refs`): `#<uuid>` references parsed from
//! node bodies, backing outgoing references and backlinks in the container
//! directory tree (siyuan-auto-directory absorption).

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;
use sqlx::PgPool;
use uuid::Uuid;

/// `#<uuid>` per `frontend/app/helpers/markdown/internal-links.ts` (UUID
/// branch only; legacy `#<digits>` SiYuan ids never resolve to Pergame rows).
static LINK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"#([0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12})")
        .expect("link regex")
});

/// Distinct link targets in a body, in first-seen order. The trailing
/// hex guard mirrors the frontend `(?![0-9a-fA-F])` (the regex crate has
/// no look-around, so the boundary is checked by hand).
pub fn extract_targets(content: &str) -> Vec<Uuid> {
    let bytes = content.as_bytes();
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for cap in LINK_RE.captures_iter(content) {
        let m = cap.get(0).map(|m| m.end()).unwrap_or(0);
        if m < bytes.len() && bytes[m].is_ascii_hexdigit() {
            continue;
        }
        let raw = cap.get(1).map(|m| m.as_str()).unwrap_or_default();
        if let Ok(id) = Uuid::parse_str(raw)
            && seen.insert(id)
        {
            out.push(id);
        }
    }
    out
}

/// `#annotation:<uuid>` per the PDF side panel (docs link to annotations;
/// backlinks resolve through `annotation_refs`). Same boundary rule as
/// `extract_targets`: a trailing hex char voids the match.
static ANN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"#annotation:([0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12})")
        .expect("annotation link regex")
});

/// Distinct annotation targets in a body, in first-seen order.
pub fn extract_annotation_targets(content: &str) -> Vec<Uuid> {
    let bytes = content.as_bytes();
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for cap in ANN_RE.captures_iter(content) {
        let m = cap.get(0).map(|m| m.end()).unwrap_or(0);
        if bytes.get(m).is_some_and(|b| b.is_ascii_hexdigit()) {
            continue;
        }
        let raw = cap.get(1).map(|m| m.as_str()).unwrap_or_default();
        if let Ok(id) = Uuid::parse_str(raw)
            && seen.insert(id)
        {
            out.push(id);
        }
    }
    out
}
/// Resources carry a filename in `content`, not markdown — skipped, as are
/// self-links and targets that are not live nodes.
pub async fn reconcile_refs(
    pool: &PgPool,
    source_id: Uuid,
    role: i32,
    content: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM node_refs WHERE source_id = $1")
        .bind(source_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM annotation_refs WHERE source_id = $1")
        .bind(source_id)
        .execute(pool)
        .await?;
    let body = content.unwrap_or_default();
    if role == 4 || body.is_empty() || !body.contains('#') {
        return Ok(());
    }
    let targets: Vec<Uuid> = extract_targets(body)
        .into_iter()
        .filter(|id| *id != source_id)
        .collect();
    if !targets.is_empty() {
        sqlx::query(
            r#"INSERT INTO node_refs (source_id, target_id)
               SELECT $1, t.id FROM nodes t
               WHERE t.id = ANY($2) AND t.deleted_at IS NULL
               ON CONFLICT DO NOTHING"#,
        )
        .bind(source_id)
        .bind(&targets)
        .execute(pool)
        .await?;
    }
    let anns: Vec<Uuid> = extract_annotation_targets(body);
    if !anns.is_empty() {
        sqlx::query(
            r#"INSERT INTO annotation_refs (source_id, annotation_id)
               SELECT $1, a.id FROM pdf_annotations a
               WHERE a.id = ANY($2)
               ON CONFLICT DO NOTHING"#,
        )
        .bind(source_id)
        .bind(&anns)
        .execute(pool)
        .await?;
    }
    Ok(())
}

/// Backfill edges for bulk-inserted rows (restore bypasses `insert`); same
/// regex as migration 0014, scoped to the restored ids. Runs inside the
/// caller's transaction.
pub async fn backfill_refs_for_ids<'e, E>(exec: E, ids: &[Uuid]) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    if ids.is_empty() {
        return Ok(());
    }
    // One statement (two CTE inserts): the generic executor moves on use.
    sqlx::query(
        r#"WITH ins_node AS (
               INSERT INTO node_refs (source_id, target_id)
               SELECT DISTINCT n.id, m.hit::uuid
               FROM nodes n
               CROSS JOIN LATERAL (
                   SELECT (regexp_matches(n.content, '#([0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12})', 'g'))[1] AS hit
               ) m
               JOIN nodes t ON t.id = m.hit::uuid AND t.deleted_at IS NULL
               WHERE n.id = ANY($1) AND n.content IS NOT NULL AND n.role <> 4 AND n.deleted_at IS NULL
               ON CONFLICT DO NOTHING
               RETURNING 1
           ),
           ins_ann AS (
               INSERT INTO annotation_refs (source_id, annotation_id)
               SELECT DISTINCT n.id, m.hit::uuid
               FROM nodes n
               CROSS JOIN LATERAL (
                   SELECT (regexp_matches(n.content, '#annotation:([0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12})', 'g'))[1] AS hit
               ) m
               JOIN pdf_annotations a ON a.id = m.hit::uuid
               WHERE n.id = ANY($1) AND n.content IS NOT NULL AND n.role <> 4 AND n.deleted_at IS NULL
               ON CONFLICT DO NOTHING
               RETURNING 1
           )
           SELECT 1"#,
    )
    .bind(ids)
    .execute(exec)
    .await?;
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
/// One link endpoint for directory rows (name/role/parent for display).
pub struct RefNode {
    pub id: Uuid,
    pub name: String,
    pub role: i32,
    pub parent_id: Option<Uuid>,
    pub updated_timestamp: i64,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
/// Both directions for one node (empty vectors when unlinked).
pub struct NodeRefs {
    pub outgoing: Vec<RefNode>,
    pub incoming: Vec<RefNode>,
}

#[derive(sqlx::FromRow)]
struct RefRow {
    anchor_id: Uuid,
    outgoing: bool,
    id: Uuid,
    name: String,
    role: i32,
    parent_id: Option<Uuid>,
    updated_timestamp: i64,
}

/// Both directions for up to a page of anchors, endpoints limited to rows
/// the caller may see (owned tree + granted tree, live only).
pub async fn refs_for(
    pool: &PgPool,
    user_id: Uuid,
    ids: &[Uuid],
) -> Result<HashMap<Uuid, NodeRefs>, sqlx::Error> {
    let mut out: HashMap<Uuid, NodeRefs> = ids.iter().map(|id| (*id, NodeRefs::default())).collect();
    if ids.is_empty() {
        return Ok(out);
    }
    let rows = sqlx::query_as::<_, RefRow>(
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
        edges AS (
            SELECT r.source_id AS anchor_id, TRUE AS outgoing, r.target_id AS other_id
            FROM node_refs r WHERE r.source_id = ANY($2)
            UNION ALL
            SELECT r.target_id AS anchor_id, FALSE AS outgoing, r.source_id AS other_id
            FROM node_refs r WHERE r.target_id = ANY($2)
        )
        SELECT e.anchor_id, e.outgoing, n.id, n.name, n.role, n.parent_id, n.updated_timestamp
        FROM edges e
        JOIN nodes n ON n.id = e.other_id
        JOIN visible_ids v ON v.id = n.id
        WHERE n.deleted_at IS NULL
        ORDER BY n.name ASC, n.id ASC"#,
    )
    .bind(user_id)
    .bind(ids)
    .fetch_all(pool)
    .await?;
    for r in rows {
        let node = RefNode {
            id: r.id,
            name: r.name,
            role: r.role,
            parent_id: r.parent_id,
            updated_timestamp: r.updated_timestamp,
        };
        if let Some(entry) = out.get_mut(&r.anchor_id) {
            if r.outgoing {
                entry.outgoing.push(node);
            } else {
                entry.incoming.push(node);
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_uuid_links_in_order() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let body = format!("see #{a} and #{b}, again #{a}; legacy #1234567890123456 ignored");
        assert_eq!(extract_targets(&body), vec![a, b]);
    }

    #[test]
    fn rejects_malformed_and_empty() {
        assert!(extract_targets("no links here").is_empty());
        assert!(extract_targets("#not-a-uuid #12345").is_empty());
        // 35 hex chars: not a UUID.
        assert!(extract_targets("#12345678-1234-1234-1234-12345678901").is_empty());
    }
}
