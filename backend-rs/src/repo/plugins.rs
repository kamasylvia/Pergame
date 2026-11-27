//! Plugin registry queries (PS-D): scoped installs, per-user grants, plugin KV.
//!
//! Visibility: `owner_id IS NULL` = global (admin-installed, everyone sees
//! it), otherwise the row belongs to exactly one user. Slugs repeat across
//! scopes, so every lookup is scope-aware; when a user forked a global slug,
//! their own row wins (own-first resolution).

use sqlx::PgPool;
use uuid::Uuid;

/// One installed plugin row.
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct PluginRow {
    pub seq: i64,
    pub id: String,
    pub name: String,
    pub version: String,
    pub api_version: String,
    pub entry: String,
    pub capabilities: Vec<String>,
    pub manifest: serde_json::Value,
    pub artifact_prefix: String,
    pub enabled: bool,
    pub owner_id: Option<Uuid>,
    pub installed_by: Option<Uuid>,
    pub created_timestamp: i64,
    pub updated_timestamp: i64,
}

/// Install or replace a plugin in one scope (same slug = new version).
/// `owner` None = global, Some(user) = that user's private copy.
#[allow(clippy::too_many_arguments)]
pub async fn upsert(
    pool: &PgPool,
    owner: Option<Uuid>,
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
    // Partial unique indexes need partial conflict targets.
    if owner.is_none() {
        sqlx::query(
            r#"INSERT INTO plugins (owner_id, id, name, version, api_version, entry, capabilities, manifest,
                    artifact_prefix, enabled, installed_by, created_timestamp, updated_timestamp)
               VALUES (NULL,$1,$2,$3,$4,$5,$6,$7,$8,TRUE,$9,$10,$10)
               ON CONFLICT (id) WHERE owner_id IS NULL DO UPDATE SET
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
    } else {
        sqlx::query(
            r#"INSERT INTO plugins (owner_id, id, name, version, api_version, entry, capabilities, manifest,
                    artifact_prefix, enabled, installed_by, created_timestamp, updated_timestamp)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,TRUE,$10,$11,$11)
               ON CONFLICT (owner_id, id) WHERE owner_id IS NOT NULL DO UPDATE SET
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
        .bind(owner)
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
}

/// Rows visible to a user: their own rows (any state, so they can manage
/// disabled ones) plus enabled globals. A forked slug shows once: the
/// user's own row shadows the global one.
pub async fn list_visible(pool: &PgPool, user_id: Uuid) -> Result<Vec<PluginRow>, sqlx::Error> {
    sqlx::query_as::<_, PluginRow>(
        "SELECT DISTINCT ON (id) * FROM plugins
         WHERE owner_id = $1 OR (owner_id IS NULL AND enabled)
         ORDER BY id, owner_id NULLS LAST, created_timestamp",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Every installed plugin, oldest first (admin view).
pub async fn list_all(pool: &PgPool) -> Result<Vec<PluginRow>, sqlx::Error> {
    sqlx::query_as::<_, PluginRow>("SELECT * FROM plugins ORDER BY created_timestamp, id")
        .fetch_all(pool)
        .await
}

/// One visible plugin by slug: the user's own row wins over the global one.
pub async fn get_visible(
    pool: &PgPool,
    user_id: Uuid,
    slug: &str,
) -> Result<Option<PluginRow>, sqlx::Error> {
    sqlx::query_as::<_, PluginRow>(
        "SELECT * FROM plugins WHERE id = $1 AND (owner_id = $2 OR owner_id IS NULL)
         ORDER BY owner_id NULLS LAST LIMIT 1",
    )
    .bind(slug)
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

/// One row by scope: owner None = the global row, Some = that user's row.
pub async fn get_scoped(
    pool: &PgPool,
    owner: Option<Uuid>,
    slug: &str,
) -> Result<Option<PluginRow>, sqlx::Error> {
    sqlx::query_as::<_, PluginRow>(
        "SELECT * FROM plugins WHERE id = $1 AND owner_id IS NOT DISTINCT FROM $2",
    )
    .bind(slug)
    .bind(owner)
    .fetch_optional(pool)
    .await
}

/// How many private plugins a user owns (install cap).
pub async fn count_owned(pool: &PgPool, user_id: Uuid) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>("SELECT count(*) FROM plugins WHERE owner_id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
}

/// Artifact prefixes of a user's private plugins (account-deletion sweep).
pub async fn owned_prefixes(pool: &PgPool, user_id: Uuid) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>("SELECT artifact_prefix FROM plugins WHERE owner_id = $1")
        .bind(user_id)
        .fetch_all(pool)
        .await
}

/// Enable/disable one row by surrogate key.
pub async fn set_enabled(pool: &PgPool, seq: i64, enabled: bool, now: i64) -> Result<bool, sqlx::Error> {
    sqlx::query("UPDATE plugins SET enabled = $2, updated_timestamp = $3 WHERE seq = $1")
        .bind(seq)
        .bind(enabled)
        .bind(now)
        .execute(pool)
        .await
        .map(|r| r.rows_affected() > 0)
}

/// Remove one row by surrogate key (grants and KV cascade).
pub async fn delete(pool: &PgPool, seq: i64) -> Result<bool, sqlx::Error> {
    sqlx::query("DELETE FROM plugins WHERE seq = $1")
        .bind(seq)
        .execute(pool)
        .await
        .map(|r| r.rows_affected() > 0)
}

/// Capabilities one user has approved for one plugin row.
pub async fn grants_for(
    pool: &PgPool,
    user_id: Uuid,
    plugin_seq: i64,
) -> Result<Vec<String>, sqlx::Error> {
    let row: Option<(Vec<String>,)> = sqlx::query_as(
        "SELECT capabilities FROM plugin_grants WHERE user_id = $1 AND plugin_seq = $2",
    )
    .bind(user_id)
    .bind(plugin_seq)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(caps,)| caps).unwrap_or_default())
}

/// Every grant of one user, keyed by plugin row (loader boot path).
pub async fn grants_for_user(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<(i64, Vec<String>)>, sqlx::Error> {
    sqlx::query_as("SELECT plugin_seq, capabilities FROM plugin_grants WHERE user_id = $1")
        .bind(user_id)
        .fetch_all(pool)
        .await
}

/// Replace a user's approved capabilities for one plugin row.
pub async fn set_grants(
    pool: &PgPool,
    user_id: Uuid,
    plugin_seq: i64,
    capabilities: &[String],
    now: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO plugin_grants (user_id, plugin_seq, capabilities, updated_timestamp)
           VALUES ($1,$2,$3,$4)
           ON CONFLICT (user_id, plugin_seq) DO UPDATE SET
             capabilities = EXCLUDED.capabilities,
             updated_timestamp = EXCLUDED.updated_timestamp"#,
    )
    .bind(user_id)
    .bind(plugin_seq)
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
    plugin_seq: i64,
    key: &str,
) -> Result<Option<serde_json::Value>, sqlx::Error> {
    let row: Option<(serde_json::Value,)> = sqlx::query_as(
        "SELECT value FROM plugin_kv WHERE user_id = $1 AND plugin_seq = $2 AND key = $3",
    )
    .bind(user_id)
    .bind(plugin_seq)
    .bind(key)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(v,)| v))
}

/// Whole namespace of one plugin row (settings UI / plugin boot).
pub async fn kv_list(
    pool: &PgPool,
    user_id: Uuid,
    plugin_seq: i64,
) -> Result<Vec<(String, serde_json::Value)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT key, value FROM plugin_kv WHERE user_id = $1 AND plugin_seq = $2 ORDER BY key",
    )
    .bind(user_id)
    .bind(plugin_seq)
    .fetch_all(pool)
    .await
}

/// Write one KV entry.
pub async fn kv_set(
    pool: &PgPool,
    user_id: Uuid,
    plugin_seq: i64,
    key: &str,
    value: &serde_json::Value,
    now: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO plugin_kv (user_id, plugin_seq, key, value, updated_timestamp)
           VALUES ($1,$2,$3,$4,$5)
           ON CONFLICT (user_id, plugin_seq, key) DO UPDATE SET
             value = EXCLUDED.value,
             updated_timestamp = EXCLUDED.updated_timestamp"#,
    )
    .bind(user_id)
    .bind(plugin_seq)
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
    plugin_seq: i64,
    key: &str,
) -> Result<bool, sqlx::Error> {
    sqlx::query("DELETE FROM plugin_kv WHERE user_id = $1 AND plugin_seq = $2 AND key = $3")
        .bind(user_id)
        .bind(plugin_seq)
        .bind(key)
        .execute(pool)
        .await
        .map(|r| r.rows_affected() > 0)
}
