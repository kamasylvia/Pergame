//! Node soft-link queries (sqlx; spec .work/features/note-links.md).
//!
//! A link row = one extra tree position for a note whose canonical parent is
//! nodes.parent_id. Cycle rule: the link's parent must not be the note itself
//! or any of its descendants (checked before insert).

use sqlx::PgPool;

use crate::models::LinkRow;

pub async fn create(
    pool: &PgPool,
    id: i64,
    parent_id: i64,
    node_id: i64,
    order: i32,
    now: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO node_links (id, parent_id, node_id, \"order\", created_timestamp)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(id)
    .bind(parent_id)
    .bind(node_id)
    .bind(order)
    .bind(now)
    .execute(pool)
    .await
    .map(|_| ())
}

pub async fn delete(pool: &PgPool, id: i64) -> Result<bool, sqlx::Error> {
    let res = sqlx::query("DELETE FROM node_links WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn get_by_id(pool: &PgPool, id: i64) -> Result<Option<LinkRow>, sqlx::Error> {
    sqlx::query_as::<_, LinkRow>(
        "SELECT id, parent_id, node_id, created_timestamp, \"order\" FROM node_links WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn links_of_node(pool: &PgPool, node_id: i64) -> Result<Vec<LinkRow>, sqlx::Error> {
    sqlx::query_as::<_, LinkRow>(
        "SELECT id, parent_id, node_id, created_timestamp, \"order\" FROM node_links WHERE node_id = $1",
    )
    .bind(node_id)
    .fetch_all(pool)
    .await
}

/// All link rows anchored on the user's own nodes (v1 scope: private tree;
/// shared/team links join through node ownership like the backup export).
pub async fn list_by_user(pool: &PgPool, user_id: i64) -> Result<Vec<LinkRow>, sqlx::Error> {
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
    node_id: i64,
    candidate_id: i64,
) -> Result<bool, sqlx::Error> {
    let hit: Option<(i64,)> = sqlx::query_as(
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
