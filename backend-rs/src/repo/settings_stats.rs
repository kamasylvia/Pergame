//! User-settings + stats queries (sqlx; Go repositories/{user_settings,
//! stats}.repository.go parity, PostgreSQL dialect).

use sqlx::PgPool;

// ------------------------------------------------------------------ settings

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct SettingsRow {
    pub user_id: i64,
    pub general: serde_json::Value,
    pub editor: serde_json::Value,
    pub advanced: serde_json::Value,
}

pub async fn get_settings(pool: &PgPool, user_id: i64) -> Result<Option<SettingsRow>, sqlx::Error> {
    sqlx::query_as::<_, SettingsRow>(
        "SELECT user_id, general, editor, advanced FROM user_settings WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

/// Go Upsert parity: INSERT ... ON CONFLICT (user_id) DO UPDATE.
pub async fn upsert_settings(
    pool: &PgPool,
    user_id: i64,
    general: &serde_json::Value,
    editor: &serde_json::Value,
    advanced: &serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO user_settings (user_id, general, editor, advanced)
           VALUES ($1, $2, $3, $4)
           ON CONFLICT (user_id) DO UPDATE
           SET general = EXCLUDED.general, editor = EXCLUDED.editor, advanced = EXCLUDED.advanced"#,
    )
    .bind(user_id)
    .bind(general)
    .bind(editor)
    .bind(advanced)
    .execute(pool)
    .await
    .map(|_| ())
}

// --------------------------------------------------------------------- stats

pub async fn users_total(pool: &PgPool) -> Result<i64, sqlx::Error> {
    let (v,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;
    Ok(v)
}

pub async fn nodes_total(pool: &PgPool) -> Result<i64, sqlx::Error> {
    let (v,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM nodes")
        .fetch_one(pool)
        .await?;
    Ok(v)
}

pub async fn nodes_total_size(pool: &PgPool) -> Result<i64, sqlx::Error> {
    let (v,): (i64,) = sqlx::query_as("SELECT COALESCE(SUM(size), 0) FROM nodes")
        .fetch_one(pool)
        .await?;
    Ok(v)
}

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct MonthlyCount {
    pub month: String,
    pub count: i64,
}

pub async fn users_monthly_growth(
    pool: &PgPool,
    since: i64,
) -> Result<Vec<MonthlyCount>, sqlx::Error> {
    sqlx::query_as::<_, MonthlyCount>(
        r#"SELECT to_char(to_timestamp(created_timestamp / 1000), 'YYYY-MM') AS month,
                  COUNT(*) AS count
           FROM users
           WHERE created_timestamp >= $1
           GROUP BY month ORDER BY month ASC"#,
    )
    .bind(since)
    .fetch_all(pool)
    .await
}

pub async fn nodes_monthly_growth(
    pool: &PgPool,
    since: i64,
) -> Result<Vec<MonthlyCount>, sqlx::Error> {
    sqlx::query_as::<_, MonthlyCount>(
        r#"SELECT to_char(to_timestamp(created_timestamp / 1000), 'YYYY-MM') AS month,
                  COUNT(*) AS count
           FROM nodes
           WHERE created_timestamp >= $1
           GROUP BY month ORDER BY month ASC"#,
    )
    .bind(since)
    .fetch_all(pool)
    .await
}

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct NodeOwnerStats {
    pub user_id: i64,
    pub username: String,
    pub node_count: i64,
    pub total_size: i64,
}

async fn top_users(
    pool: &PgPool,
    limit: i32,
    by: TopOrder,
) -> Result<Vec<NodeOwnerStats>, sqlx::Error> {
    let sql = match by {
        TopOrder::NodeCount => {
            r#"SELECT n.user_id AS user_id, u.username AS username,
                  COUNT(n.id) AS node_count, COALESCE(SUM(n.size), 0) AS total_size
           FROM nodes n
           JOIN users u ON u.id = n.user_id
           GROUP BY n.user_id, u.username
           ORDER BY node_count DESC, total_size DESC
           LIMIT $1"#
        }
        TopOrder::TotalSize => {
            r#"SELECT n.user_id AS user_id, u.username AS username,
                  COUNT(n.id) AS node_count, COALESCE(SUM(n.size), 0) AS total_size
           FROM nodes n
           JOIN users u ON u.id = n.user_id
           GROUP BY n.user_id, u.username
           ORDER BY total_size DESC, node_count DESC
           LIMIT $1"#
        }
    };
    sqlx::query_as::<_, NodeOwnerStats>(sql)
        .bind(limit)
        .fetch_all(pool)
        .await
}

#[derive(Debug, Clone, Copy)]
enum TopOrder {
    NodeCount,
    TotalSize,
}

pub async fn top_users_by_node_count(
    pool: &PgPool,
    limit: i32,
) -> Result<Vec<NodeOwnerStats>, sqlx::Error> {
    top_users(pool, limit, TopOrder::NodeCount).await
}

pub async fn top_users_by_node_size(
    pool: &PgPool,
    limit: i32,
) -> Result<Vec<NodeOwnerStats>, sqlx::Error> {
    top_users(pool, limit, TopOrder::TotalSize).await
}
