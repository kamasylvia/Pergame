// The schema/table identifiers embedded via format! are validated in the
// constructor ([A-Za-z0-9_] only); data values are always bound parameters.
// sqlx's SqlSafeStr audit fires on dynamic table/schema names by design —
// AssertSqlSafe is the documented escape hatch for validated identifiers.

//! External PostgreSQL fan-out target: single-table object store
//! (`<schema>.pergame_backup_objects`), mirroring the fork's pg.go design.
//! The target PG is a *different* database than the app's own — connected
//! via its own ad-hoc sqlx pool per operation batch (cached upstream).

use sqlx::AssertSqlSafe;
use sqlx::postgres::PgPoolOptions;

use super::{Fingerprint, ObjectRow, PgStoreParams, internal, join_prefix};

pub struct PgTarget {
    pool: sqlx::PgPool,
    schema: String,
}

impl PgTarget {
    pub async fn new(p: &PgStoreParams) -> Result<Self, crate::error::ApiError> {
        let pool = PgPoolOptions::new()
            .max_connections(2)
            .acquire_timeout(std::time::Duration::from_secs(10))
            .connect(&p.dsn)
            .await
            .map_err(|e| internal(format!("pg target connect: {e}")))?;
        let schema = p.schema.trim().trim_matches('"').to_string();
        if schema.is_empty()
            || !schema
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return Err(crate::error::ApiError::BadRequest(
                "pg target schema must be [A-Za-z0-9_]".into(),
            ));
        }
        let t = Self { pool, schema };
        t.ensure_table().await?;
        Ok(t)
    }

    fn table(&self) -> String {
        format!("{}.pergame_backup_objects", self.schema)
    }

    async fn ensure_table(&self) -> Result<(), crate::error::ApiError> {
        sqlx::query(AssertSqlSafe(format!(
            "CREATE SCHEMA IF NOT EXISTS \"{}\"",
            self.schema
        )))
        .execute(&self.pool)
        .await
        .map_err(|e| internal(format!("pg target schema: {e}")))?;
        sqlx::query(AssertSqlSafe(format!(
            "CREATE TABLE IF NOT EXISTS {} (\
             path TEXT PRIMARY KEY, \
             data BYTEA NOT NULL, \
             size BIGINT NOT NULL, \
             updated_at TIMESTAMPTZ NOT NULL DEFAULT now())",
            self.table()
        )))
        .execute(&self.pool)
        .await
        .map_err(|e| internal(format!("pg target table: {e}")))?;
        Ok(())
    }

    /// Physical identity: cluster system_identifier + database name + schema.
    /// PG >= 14 exposes pg_control_system(); older servers fall back to the
    /// database oid (weaker but still stable per-database).
    pub async fn fingerprint_parts(
        &self,
    ) -> Result<(String, String, String), crate::error::ApiError> {
        let static_sys: Result<(String,), _> =
            sqlx::query_as("SELECT system_identifier::text FROM pg_control_system()")
                .fetch_one(&self.pool)
                .await;
        let sys: (String,) = match static_sys {
            Ok(v) => v,
            Err(_) => sqlx::query_as(
                "SELECT oid::text FROM pg_database WHERE datname = current_database()",
            )
            .fetch_one(&self.pool)
            .await
            .unwrap_or(("0".into(),)),
        };
        let db: (String,) = sqlx::query_as("SELECT current_database()")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| internal(format!("pg fingerprint db: {e}")))?;
        Ok((sys.0, db.0, self.schema.clone()))
    }

    pub async fn probe(&self) -> Result<(), crate::error::ApiError> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(|e| internal(format!("pg target probe: {e}")))
    }

    pub async fn has(&self, path: &str) -> Result<bool, crate::error::ApiError> {
        let (n,): (i64,) = sqlx::query_as(AssertSqlSafe(format!(
            "SELECT count(*) FROM {} WHERE path = $1",
            self.table()
        )))
        .bind(path)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| internal(format!("pg target has: {e}")))?;
        Ok(n > 0)
    }

    pub async fn put(&self, path: &str, bytes: Vec<u8>) -> Result<(), crate::error::ApiError> {
        sqlx::query(AssertSqlSafe(format!(
            "INSERT INTO {} (path, data, size, updated_at) VALUES ($1,$2,$3,now()) \
             ON CONFLICT (path) DO UPDATE SET data = EXCLUDED.data, \
             size = EXCLUDED.size, updated_at = now()",
            self.table()
        )))
        .bind(path)
        .bind(&bytes)
        .bind(bytes.len() as i64)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(|e| internal(format!("pg target put {path}: {e}")))
    }

    pub async fn get(&self, path: &str) -> Result<Vec<u8>, crate::error::ApiError> {
        let (data,): (Vec<u8>,) = sqlx::query_as(AssertSqlSafe(format!(
            "SELECT data FROM {} WHERE path = $1",
            self.table()
        )))
        .bind(path)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| internal(format!("pg target get: {e}")))?
        .ok_or(crate::error::ApiError::NotFound("object not found".into()))?;
        Ok(data)
    }

    pub async fn delete(&self, path: &str) -> Result<bool, crate::error::ApiError> {
        let res = sqlx::query(AssertSqlSafe(format!(
            "DELETE FROM {} WHERE path = $1",
            self.table()
        )))
        .bind(path)
        .execute(&self.pool)
        .await
        .map_err(|e| internal(format!("pg target delete: {e}")))?;
        Ok(res.rows_affected() > 0)
    }

    pub async fn list(&self, path: &str) -> Result<Vec<ObjectRow>, crate::error::ApiError> {
        let like = format!("{}%", join_prefix("", path));
        let rows: Vec<(String, i64, i64)> = sqlx::query_as(AssertSqlSafe(format!(
            "SELECT path, size, (extract(epoch from updated_at) * 1000)::bigint \
             FROM {} WHERE path LIKE $1 ORDER BY path",
            self.table()
        )))
        .bind(like)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| internal(format!("pg target list: {e}")))?;
        Ok(rows)
    }
}

pub fn fingerprint(_p: &PgStoreParams, parts: (String, String, String)) -> Fingerprint {
    Fingerprint(super::pg_identity(&parts.0, &parts.1, &parts.2))
}
