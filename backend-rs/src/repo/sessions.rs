//! Session read/teardown queries (sqlx; Go session.repository parity).

use sqlx::PgPool;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct SessionView {
    pub id: i64,
    pub user_id: i64,
    pub refresh_token: String,
    pub expire_token: i64,
    pub last_refresh_timestamp: i64,
    pub active: i32,
    pub ip_addr: Option<String>,
    pub user_agent: Option<String>,
    pub location: Option<String>,
    #[serde(rename = "type")]
    #[sqlx(rename = "type")]
    pub session_type: Option<String>,
    pub login_timestamp: i64,
    pub logout_timestamp: i64,
}

pub async fn list_by_user(pool: &PgPool, user_id: i64) -> Result<Vec<SessionView>, sqlx::Error> {
    sqlx::query_as::<_, SessionView>(
        "SELECT id, user_id, refresh_token, expire_token, last_refresh_timestamp, active,
                ip_addr, user_agent, location, type, login_timestamp, logout_timestamp
         FROM sessions WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

pub async fn delete_by_id_and_user(
    pool: &PgPool,
    id: i64,
    user_id: i64,
) -> Result<bool, sqlx::Error> {
    let res = sqlx::query("DELETE FROM sessions WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn delete_all_by_user(pool: &PgPool, user_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM sessions WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await
        .map(|_| ())
}
