//! Node soft-link queries (spec .work/features/note-links.md).
//!
//! A link row = one extra tree position for a note whose canonical parent is
//! nodes.parent_id. Cycle rule: the link's parent must not be the note itself
//! or any of its descendants (checked before insert).
//!
//! CRUD goes through Toasty ([`NodeLink`]); the recursive cycle check and the
//! user-wide list stay in sqlx because they need CTEs / joins that Toasty's
//! query model does not express.

use sqlx::PgPool;
use toasty::Db;
use uuid::Uuid;

use crate::models::{LinkRow, NodeLink};

/// Creates a link row. Unique-violation surfaces as `sqlx::Error` so callers
/// keep their existing 409 mapping.
pub async fn create(
    db: &mut Db,
    id: Uuid,
    parent_id: Uuid,
    node_id: Uuid,
    order: i32,
    now: i64,
) -> Result<(), String> {
    NodeLink::create()
        .id(id)
        .parent_id(parent_id)
        .node_id(node_id)
        .order(order)
        .created_timestamp(now)
        .exec(db)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Deletes one link position; false when the row did not exist.
pub async fn delete(db: &mut Db, id: Uuid) -> Result<bool, String> {
    let existed = NodeLink::filter_by_id(id)
        .first()
        .exec(db)
        .await
        .map_err(|e| e.to_string())?
        .is_some();
    if !existed {
        return Ok(false);
    }
    NodeLink::filter_by_id(id)
        .delete()
        .exec(db)
        .await
        .map_err(|e| e.to_string())?;
    Ok(true)
}

pub async fn get_by_id(db: &mut Db, id: Uuid) -> Result<Option<LinkRow>, String> {
    let row = NodeLink::filter_by_id(id)
        .first()
        .exec(db)
        .await
        .map_err(|e| e.to_string())?;
    Ok(row.map(|r| LinkRow {
        id: r.id,
        parent_id: r.parent_id,
        node_id: r.node_id,
        created_timestamp: r.created_timestamp,
        order: r.order,
    }))
}

pub async fn links_of_node(db: &mut Db, node_id: Uuid) -> Result<Vec<LinkRow>, String> {
    let rows = NodeLink::filter_by_node_id(node_id)
        .exec(db)
        .await
        .map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .map(|r| LinkRow {
            id: r.id,
            parent_id: r.parent_id,
            node_id: r.node_id,
            created_timestamp: r.created_timestamp,
            order: r.order,
        })
        .collect())
}

/// All link rows anchored on the user's own nodes (v1 scope: private tree;
/// shared/team links join through node ownership like the backup export).
pub async fn list_by_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<LinkRow>, sqlx::Error> {
    sqlx::query_as::<_, LinkRow>(
        "SELECT nl.id, nl.parent_id, nl.node_id, nl.created_timestamp, nl.\"order\" \
         FROM node_links nl JOIN nodes n ON n.id = nl.node_id \
         WHERE n.user_id = $1 ORDER BY nl.\"order\", nl.id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// True when `candidate_id` is the node itself or one of its descendants
/// (recursive walk; links do not participate — the tree is parent_id-only).
pub async fn is_self_or_descendant(
    pool: &PgPool,
    node_id: Uuid,
    candidate_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let hit: Option<(Uuid,)> = sqlx::query_as(
        r#"
        WITH RECURSIVE descendants AS (
            SELECT id FROM nodes WHERE id = $1
            UNION ALL
            SELECT n.id FROM nodes n JOIN descendants d ON n.parent_id = d.id
        )
        SELECT 1::bigint FROM descendants WHERE id = $2 LIMIT 1"#,
    )
    .bind(node_id)
    .bind(candidate_id)
    .fetch_optional(pool)
    .await?;
    Ok(hit.is_some())
}
