//! Plugin registry queries (PS-D): installs, per-user grants, plugin KV.

use sqlx::PgPool;
use uuid::Uuid;

/// One installed plugin row.
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct PluginRow {
    pub id: String,
    pub name: String,
    pub version: String,
    pub api_version: String,
    pub entry: String,
    pub capabilities: Vec<String>,
    pub manifest: serde_json::Value,
    pub artifact_prefix: String,
    pub enabled: bool,
    pub installed_by: Option<Uuid>,
    pub created_timestamp: i64,
    pub updated_timestamp: i64,
}

/// Install or replace a plugin (same slug = new version).
#[allow(clippy::too_many_arguments)]
pub async fn upsert(
    pool: &PgPool,
    id: &str,
    name: &str,
    version: &str,
    api_version: &str,
    entry: &str,
    capabilities: &[String],
    manifest: &serde_json::Value,
    artifact_prefix: &str,
    installed_by: Option<Uuid>,
    now: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO plugins (id, name, version, api_version, entry, capabilities, manifest,
                artifact_prefix, enabled, installed_by, created_timestamp, updated_timestamp)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,TRUE,$9,$10,$10)
           ON CONFLICT (id) DO UPDATE SET
             name = EXCLUDED.name,
             version = EXCLUDED.version,
             api_version = EXCLUDED.api_version,
             entry = EXCLUDED.entry,
             capabilities = EXCLUDED.capabilities,
             manifest = EXCLUDED.manifest,
             artifact_prefix = EXCLUDED.artifact_prefix,
             enabled = TRUE,
             installed_by = EXCLUDED.installed_by,
             updated_timestamp = EXCLUDED.updated_timestamp"#,
    )
    .bind(id)
    .bind(name)
    .bind(version)
    .bind(api_version)
    .bind(entry)
    .bind(capabilities)
    .bind(manifest)
    .bind(artifact_prefix)
    .bind(installed_by)
    .bind(now)
    .execute(pool)
    .await
    .map(|_| ())
}

/// All installed plugins, oldest first.
pub async fn list(pool: &PgPool) -> Result<Vec<PluginRow>, sqlx::Error> {
    sqlx::query_as::<_, PluginRow>("SELECT * FROM plugins ORDER BY created_timestamp, id")
        .fetch_all(pool)
        .await
}

/// One plugin by slug.
pub async fn get(pool: &PgPool, id: &str) -> Result<Option<PluginRow>, sqlx::Error> {
    sqlx::query_as::<_, PluginRow>("SELECT * FROM plugins WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
}

/// Enable/disable an installed plugin.
pub async fn set_enabled(
    pool: &PgPool,
    id: &str,
    enabled: bool,
    now: i64,
) -> Result<bool, sqlx::Error> {
    sqlx::query("UPDATE plugins SET enabled = $2, updated_timestamp = $3 WHERE id = $1")
        .bind(id)
        .bind(enabled)
        .bind(now)
        .execute(pool)
        .await
        .map(|r| r.rows_affected() > 0)
}

/// Remove an installed plugin (grants and KV cascade).
pub async fn delete(pool: &PgPool, id: &str) -> Result<bool, sqlx::Error> {
    sqlx::query("DELETE FROM plugins WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map(|r| r.rows_affected() > 0)
}

/// Capabilities one user has approved for one plugin.
pub async fn grants_for(
    pool: &PgPool,
    user_id: Uuid,
    plugin_id: &str,
) -> Result<Vec<String>, sqlx::Error> {
    let row: Option<(Vec<String>,)> = sqlx::query_as(
        "SELECT capabilities FROM plugin_grants WHERE user_id = $1 AND plugin_id = $2",
    )
    .bind(user_id)
    .bind(plugin_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(caps,)| caps).unwrap_or_default())
}

/// Every grant of one user, keyed by plugin id (loader boot path).
pub async fn grants_for_user(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<(String, Vec<String>)>, sqlx::Error> {
    sqlx::query_as("SELECT plugin_id, capabilities FROM plugin_grants WHERE user_id = $1")
        .bind(user_id)
        .fetch_all(pool)
        .await
}

/// Replace a user's approved capabilities for a plugin.
pub async fn set_grants(
    pool: &PgPool,
    user_id: Uuid,
    plugin_id: &str,
    capabilities: &[String],
    now: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO plugin_grants (user_id, plugin_id, capabilities, updated_timestamp)
           VALUES ($1,$2,$3,$4)
           ON CONFLICT (user_id, plugin_id) DO UPDATE SET
             capabilities = EXCLUDED.capabilities,
             updated_timestamp = EXCLUDED.updated_timestamp"#,
    )
    .bind(user_id)
    .bind(plugin_id)
    .bind(capabilities)
    .bind(now)
    .execute(pool)
    .await
    .map(|_| ())
}

/// One KV entry.
pub async fn kv_get(
    pool: &PgPool,
    user_id: Uuid,
    plugin_id: &str,
    key: &str,
) -> Result<Option<serde_json::Value>, sqlx::Error> {
    let row: Option<(serde_json::Value,)> = sqlx::query_as(
        "SELECT value FROM plugin_kv WHERE user_id = $1 AND plugin_id = $2 AND key = $3",
    )
    .bind(user_id)
    .bind(plugin_id)
    .bind(key)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(v,)| v))
}

/// Whole namespace of one plugin (settings UI / plugin boot).
pub async fn kv_list(
    pool: &PgPool,
    user_id: Uuid,
    plugin_id: &str,
) -> Result<Vec<(String, serde_json::Value)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT key, value FROM plugin_kv WHERE user_id = $1 AND plugin_id = $2 ORDER BY key",
    )
    .bind(user_id)
    .bind(plugin_id)
    .fetch_all(pool)
    .await
}

/// Write one KV entry.
pub async fn kv_set(
    pool: &PgPool,
    user_id: Uuid,
    plugin_id: &str,
    key: &str,
    value: &serde_json::Value,
    now: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO plugin_kv (user_id, plugin_id, key, value, updated_timestamp)
           VALUES ($1,$2,$3,$4,$5)
           ON CONFLICT (user_id, plugin_id, key) DO UPDATE SET
             value = EXCLUDED.value,
             updated_timestamp = EXCLUDED.updated_timestamp"#,
    )
    .bind(user_id)
    .bind(plugin_id)
    .bind(key)
    .bind(value)
    .bind(now)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Delete one KV entry; `true` when it existed.
pub async fn kv_delete(
    pool: &PgPool,
    user_id: Uuid,
    plugin_id: &str,
    key: &str,
) -> Result<bool, sqlx::Error> {
    sqlx::query("DELETE FROM plugin_kv WHERE user_id = $1 AND plugin_id = $2 AND key = $3")
        .bind(user_id)
        .bind(plugin_id)
        .bind(key)
        .execute(pool)
        .await
        .map(|r| r.rows_affected() > 0)
}
