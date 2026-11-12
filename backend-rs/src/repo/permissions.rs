//! Permission + invitation queries (Go repositories/permission.repository.go
//! parity, PostgreSQL dialect). Permission CRUD goes through Toasty
//! (`Permission`); the subtree-recursive grant query and invitation access
//! stay in sqlx (CTE / composite tuples).

use sqlx::PgPool;
use uuid::Uuid;
use toasty::Db;

use crate::models::{NodeRow, Permission, PermissionRow};

pub async fn get_by_node(db: &mut Db, node_id: Uuid) -> Result<Vec<PermissionRow>, String> {
    let rows = Permission::filter_by_node_id(node_id)
        .exec(db)
        .await
        .map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .map(|r| PermissionRow {
            id: r.id,
            node_id: r.node_id,
            user_id: r.user_id,
            permission: r.permission,
            created_timestamp: r.created_timestamp,
        })
        .collect())
}

pub async fn get_by_node_recursive(
    pool: &PgPool,
    node_id: Uuid,
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

pub async fn get_by_id(db: &mut Db, id: Uuid) -> Result<Option<PermissionRow>, String> {
    let row = Permission::filter_by_id(id)
        .first()
        .exec(db)
        .await
        .map_err(|e| e.to_string())?;
    Ok(row.map(|r| PermissionRow {
        id: r.id,
        node_id: r.node_id,
        user_id: r.user_id,
        permission: r.permission,
        created_timestamp: r.created_timestamp,
    }))
}

pub async fn get_by_node_and_user(
    db: &mut Db,
    node_id: Uuid,
    user_id: Uuid,
) -> Result<Option<PermissionRow>, String> {
    let row = Permission::filter_by_node_id(node_id)
        .filter_by_user_id(user_id)
        .first()
        .exec(db)
        .await
        .map_err(|e| e.to_string())?;
    Ok(row.map(|r| PermissionRow {
        id: r.id,
        node_id: r.node_id,
        user_id: r.user_id,
        permission: r.permission,
        created_timestamp: r.created_timestamp,
    }))
}

pub async fn create(
    db: &mut Db,
    id: Uuid,
    node_id: Uuid,
    user_id: Uuid,
    permission: i32,
    now: i64,
) -> Result<(), String> {
    Permission::create()
        .id(id)
        .node_id(node_id)
        .user_id(user_id)
        .permission(permission)
        .created_timestamp(now)
        .exec(db)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

pub async fn update(db: &mut Db, id: Uuid, permission: i32) -> Result<(), String> {
    let mut row = Permission::filter_by_id(id)
        .first()
        .exec(db)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "permission not found".to_string())?;
    row.update()
        .permission(permission)
        .exec(db)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

pub async fn delete(db: &mut Db, id: Uuid) -> Result<(), String> {
    Permission::filter_by_id(id)
        .delete()
        .exec(db)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

pub async fn get_invitation_by_id(
    pool: &PgPool,
    id: Uuid,
) -> Result<Option<(Uuid, String, i32, Uuid, i64)>, sqlx::Error> {
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
) -> Result<Option<(Uuid, String, i32, Uuid, i64)>, sqlx::Error> {
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
    node_id: Uuid,
) -> Result<Vec<(Uuid, String, i32, Uuid, i64)>, sqlx::Error> {
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
    id: Uuid,
    code: &str,
    level: i32,
    node_id: Uuid,
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

pub async fn delete_invitation(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM node_invitations WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map(|_| ())
}

/// Node row projection used by the join flow (id/user_id/name/parent_id).
pub async fn node_brief(pool: &PgPool, node_id: Uuid) -> Result<Option<NodeRow>, sqlx::Error> {
    crate::repo::nodes::get_by_id(pool, node_id).await
}
