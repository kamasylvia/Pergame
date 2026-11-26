//! SiYuan-style PDF annotations: selection highlights with comments that
//! live in Pergame rows (the stored file bytes stay clean forever).
//! Docs link to annotations via `#annotation:<uuid>`; backlinks resolve
//! through `annotation_refs` (reconciled in refs.rs, same as node_refs).

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

/// One PDF-space rect (points, default user space, rotation-free).
#[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize)]
pub struct Quad {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl Quad {
    fn sane(&self) -> bool {
        self.x0.is_finite() && self.y0.is_finite() && self.x1.is_finite() && self.y1.is_finite()
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
/// One annotation row (quads serialize as the stored JSON array).
pub struct Annotation {
    pub id: Uuid,
    pub user_id: Uuid,
    pub resource_id: Uuid,
    pub page: i32,
    pub quads: serde_json::Value,
    pub quote: String,
    pub comment: String,
    pub color: i32,
    pub created_timestamp: i64,
    pub updated_timestamp: i64,
}

/// Validate a create payload; returns the normalized quads JSON.
pub fn checked_quads(quads: &[Quad]) -> Result<serde_json::Value, String> {
    if quads.is_empty() || quads.len().gt(&64) {
        return Err("quads must hold 1..=64 rects".to_string());
    }
    if quads.iter().any(|q| !q.sane()) {
        return Err("quads must be finite numbers".to_string());
    }
    serde_json::to_value(quads).map_err(|e| e.to_string())
}

/// Annotations of one resource, in reading order.
pub async fn list_for_resource(
    pool: &PgPool,
    resource_id: Uuid,
) -> Result<Vec<Annotation>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, user_id, resource_id, page, quads, quote, comment, color, created_timestamp, updated_timestamp FROM pdf_annotations WHERE resource_id = $1 ORDER BY page ASC, created_timestamp ASC"
    )
    .bind(resource_id)
    .fetch_all(pool)
    .await
}

pub async fn get(pool: &PgPool, id: Uuid) -> Result<Option<Annotation>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, user_id, resource_id, page, quads, quote, comment, color, created_timestamp, updated_timestamp FROM pdf_annotations WHERE id = $1"
    )
        .bind(id)
        .fetch_optional(pool)
        .await
}

/// Insert payload (bundled: clippy caps fn args at 7).
pub struct NewAnnotation<'a> {
    pub id: Uuid,
    pub user_id: Uuid,
    pub resource_id: Uuid,
    pub page: i32,
    pub quads: serde_json::Value,
    pub quote: &'a str,
    pub comment: &'a str,
    pub color: i32,
    pub now: i64,
}

pub async fn create(pool: &PgPool, ann: NewAnnotation<'_>) -> Result<Annotation, sqlx::Error> {
    sqlx::query_as(
        "INSERT INTO pdf_annotations (id, user_id, resource_id, page, quads, quote, comment, color, created_timestamp, updated_timestamp)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9)
         RETURNING id, user_id, resource_id, page, quads, quote, comment, color, created_timestamp, updated_timestamp"
    )
    .bind(ann.id)
    .bind(ann.user_id)
    .bind(ann.resource_id)
    .bind(ann.page)
    .bind(ann.quads)
    .bind(ann.quote)
    .bind(ann.comment)
    .bind(ann.color)
    .bind(ann.now)
    .fetch_one(pool)
    .await
}

pub async fn update(
    pool: &PgPool,
    id: Uuid,
    comment: &str,
    color: i32,
    now: i64,
) -> Result<Option<Annotation>, sqlx::Error> {
    sqlx::query_as(
        "UPDATE pdf_annotations SET comment = $2, color = $3, updated_timestamp = $4 WHERE id = $1 RETURNING id, user_id, resource_id, page, quads, quote, comment, color, created_timestamp, updated_timestamp"
    )
    .bind(id)
    .bind(comment)
    .bind(color)
    .bind(now)
    .fetch_optional(pool)
    .await
}

pub async fn delete(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM pdf_annotations WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Docs linking to one annotation, limited to rows the caller may see
/// (owned tree + granted tree, live only). Mirrors refs.rs visibility.
pub async fn refs(
    pool: &PgPool,
    user_id: Uuid,
    annotation_id: Uuid,
) -> Result<Vec<super::nodes::refs::RefNode>, sqlx::Error> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        name: String,
        role: i32,
        parent_id: Option<Uuid>,
        updated_timestamp: i64,
    }
    let rows = sqlx::query_as::<_, Row>(
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
        SELECT n.id, n.name, n.role, n.parent_id, n.updated_timestamp
        FROM annotation_refs r
        JOIN nodes n ON n.id = r.source_id
        JOIN visible_ids v ON v.id = n.id
        WHERE r.annotation_id = $2 AND n.deleted_at IS NULL
        ORDER BY n.updated_timestamp DESC"#,
    )
    .bind(user_id)
    .bind(annotation_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| super::nodes::refs::RefNode {
            id: r.id,
            name: r.name,
            role: r.role,
            parent_id: r.parent_id,
            updated_timestamp: r.updated_timestamp,
        })
        .collect())
}
