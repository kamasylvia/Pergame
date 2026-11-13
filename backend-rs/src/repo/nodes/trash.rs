//! Trash (soft-delete) subtree queries sharing one `deleted_at` batch stamp.

use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{NodeRow, TrashRow};

// ---------------------------------------------------------------- trash
// Soft delete (spec .work/features/trash.md): a subtree deleted in one action
// shares a single `deleted_at` stamp, so restore brings the whole batch back.

/// Ids of `node_id` plus every descendant (parent_id chain; node_links do not
/// carry content and are filtered at read time instead of being copied here).
pub async fn subtree_ids(pool: &PgPool, node_id: Uuid) -> Result<Vec<Uuid>, sqlx::Error> {
    let rows: Vec<(Uuid,)> = sqlx::query_as(
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
    node_id: Uuid,
    now: i64,
) -> Result<(i64, u64), sqlx::Error> {
    let ids = subtree_ids(pool, node_id).await?;
    if ids.is_empty() {
        return Ok((now, 0));
    }
    let affected =
        sqlx::query("UPDATE nodes SET deleted_at = $2 WHERE id = ANY($1) AND deleted_at IS NULL")
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
pub async fn list_trash(pool: &PgPool, user_id: Uuid) -> Result<Vec<TrashRow>, sqlx::Error> {
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

    let restored =
        sqlx::query("UPDATE nodes SET deleted_at = NULL WHERE id = ANY($1) AND deleted_at = $2")
            .bind(&ids)
            .bind(deleted_at)
            .execute(pool)
            .await?
            .rows_affected();
    Ok((restored, reparented))
}

/// Physical delete of a trashed subtree (S3 cleanup happens in the route,
/// before this call). Returns the removed node count.
pub async fn purge_subtree(pool: &PgPool, node_id: Uuid) -> Result<u64, sqlx::Error> {
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
pub async fn trash_roots(pool: &PgPool, user_id: Uuid) -> Result<Vec<Uuid>, sqlx::Error> {
    let rows: Vec<(Uuid,)> = sqlx::query_as(
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
