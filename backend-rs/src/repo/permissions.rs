//! Permission + invitation queries (sqlx; Go repositories/permission.repository.go
//! parity, PostgreSQL dialect).

use sqlx::PgPool;

use crate::models::{NodeRow, PermissionRow};

pub async fn get_by_node(pool: &PgPool, node_id: i64) -> Result<Vec<PermissionRow>, sqlx::Error> {
    sqlx::query_as::<_, PermissionRow>(
        "SELECT id, node_id, user_id, permission, created_timestamp
         FROM permissions WHERE node_id = $1",
    )
    .bind(node_id)
    .fetch_all(pool)
    .await
}

pub async fn get_by_node_recursive(
    pool: &PgPool,
    node_id: i64,
) -> Result<Vec<PermissionRow>, sqlx::Error> {
    sqlx::query_as::<_, PermissionRow>(
        r#"
        WITH RECURSIVE descendants AS (
            SELECT id FROM nodes WHERE id = $1 AND deleted_at IS NULL
            UNION ALL
            SELECT n.id FROM nodes n
            INNER JOIN descendants d ON n.parent_id = d.id
            WHERE n.deleted_at IS NULL
        )
        SELECT p.id, p.node_id, p.user_id, p.permission, p.created_timestamp
        FROM permissions p
        INNER JOIN descendants des ON p.node_id = des.id"#,
    )
    .bind(node_id)
    .fetch_all(pool)
    .await
}

pub async fn get_by_id(pool: &PgPool, id: i64) -> Result<Option<PermissionRow>, sqlx::Error> {
    sqlx::query_as::<_, PermissionRow>(
        "SELECT id, node_id, user_id, permission, created_timestamp
         FROM permissions WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn get_by_node_and_user(
    pool: &PgPool,
    node_id: i64,
    user_id: i64,
) -> Result<Option<PermissionRow>, sqlx::Error> {
    sqlx::query_as::<_, PermissionRow>(
        "SELECT id, node_id, user_id, permission, created_timestamp
         FROM permissions WHERE node_id = $1 AND user_id = $2",
    )
    .bind(node_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

pub async fn create(
    pool: &PgPool,
    id: i64,
    node_id: i64,
    user_id: i64,
    permission: i32,
    now: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO permissions (id, node_id, user_id, permission, created_timestamp)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(id)
    .bind(node_id)
    .bind(user_id)
    .bind(permission)
    .bind(now)
    .execute(pool)
    .await
    .map(|_| ())
}

pub async fn update(pool: &PgPool, id: i64, permission: i32) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE permissions SET permission = $1 WHERE id = $2")
        .bind(permission)
        .bind(id)
        .execute(pool)
        .await
        .map(|_| ())
}

pub async fn delete(pool: &PgPool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM permissions WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map(|_| ())
}

pub async fn get_invitation_by_id(
    pool: &PgPool,
    id: i64,
) -> Result<Option<(i64, String, i32, i64, i64)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, invitation_code, permission_level, node_id, created_timestamp
         FROM node_invitations WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn get_invitation_by_code(
    pool: &PgPool,
    code: &str,
) -> Result<Option<(i64, String, i32, i64, i64)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, invitation_code, permission_level, node_id, created_timestamp
         FROM node_invitations WHERE invitation_code = $1",
    )
    .bind(code)
    .fetch_optional(pool)
    .await
}

pub async fn invitations_by_node(
    pool: &PgPool,
    node_id: i64,
) -> Result<Vec<(i64, String, i32, i64, i64)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, invitation_code, permission_level, node_id, created_timestamp
         FROM node_invitations WHERE node_id = $1",
    )
    .bind(node_id)
    .fetch_all(pool)
    .await
}

pub async fn create_invitation(
    pool: &PgPool,
    id: i64,
    code: &str,
    level: i32,
    node_id: i64,
    now: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO node_invitations (id, invitation_code, permission_level, node_id, created_timestamp)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(id)
    .bind(code)
    .bind(level)
    .bind(node_id)
    .bind(now)
    .execute(pool)
    .await
    .map(|_| ())
}

pub async fn delete_invitation(pool: &PgPool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM node_invitations WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map(|_| ())
}

/// Node row projection used by the join flow (id/user_id/name/parent_id).
pub async fn node_brief(pool: &PgPool, node_id: i64) -> Result<Option<NodeRow>, sqlx::Error> {
    crate::repo::nodes::get_by_id(pool, node_id).await
}
