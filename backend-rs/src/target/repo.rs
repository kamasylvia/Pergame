//! Backup-target persistence + gates + fan-out engine (spec §14.4/14.5).
//!
//! Invariants enforced here (fork deletecheck.go/editCheck parity):
//! - A target is only deletable when provably emptied, or when it is
//!   unreachable but a fingerprint-identical sibling config is reachable and
//!   provably emptied (the "IP vs hostname" duplicate case).
//! - Connection-field edits require the same proof; display/retention edits
//!   skip probing.
//! - Credentials are masked on read and restored from the stored row when the
//!   client echoes `***` back on write.

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::config::{BackupTargetRow, ProbeOutcome};
use crate::error::ApiError;

pub const MASK: &str = "***";

/// Wire model for target CRUD: same fields as the row, secrets masked.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetInput {
    pub kind: String,
    pub name: String,
    pub endpoint: String,
    #[serde(default)]
    pub bucket: String,
    #[serde(default)]
    pub prefix: String,
    #[serde(default = "mask_default")]
    pub access_key: String,
    #[serde(default = "mask_default")]
    pub secret_key: String,
    #[serde(default = "default_true")]
    pub secure: bool,
    #[serde(default = "default_region")]
    pub region: String,
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default = "default_keep")]
    pub keep_last: i32,
    #[serde(default)]
    pub max_age_days: i32,
    #[serde(default)]
    pub enabled: bool,
    /// Owning user ids (empty = unassigned, cannot enable).
    #[serde(default)]
    pub users: Vec<i64>,
}

fn mask_default() -> String {
    MASK.into()
}
fn default_true() -> bool {
    true
}
fn default_region() -> String {
    "us-east-1".into()
}
fn default_provider() -> String {
    "Other".into()
}
fn default_keep() -> i32 {
    10
}

/// Masks a stored row for API output.
pub fn mask_row(row: &BackupTargetRow) -> serde_json::Value {
    serde_json::json!({
        "id": row.id,
        "kind": row.kind,
        "name": row.name,
        "endpoint": row.endpoint,
        "bucket": row.bucket,
        "prefix": row.prefix,
        "access_key": if row.access_key.is_empty() { "" } else { MASK },
        "secret_key": if row.secret_key.is_empty() { "" } else { MASK },
        "secure": row.secure,
        "region": row.region,
        "provider": row.provider,
        "keep_last": row.keep_last,
        "max_age_days": row.max_age_days,
        "enabled": row.enabled,
        "last_result": row.last_result,
        "users": row.users,
        "created_at": row.created_at,
        "updated_at": row.updated_at,
    })
}

// The row carries `users` now; keep it in the struct via a thin alias so the
// sqlx FromRow derive in config.rs stays in one place.
pub async fn list_targets(pool: &PgPool) -> Result<Vec<BackupTargetRow>, ApiError> {
    sqlx::query_as::<_, BackupTargetRow>("SELECT * FROM backup_targets ORDER BY created_at")
        .fetch_all(pool)
        .await
        .map_err(ApiError::from)
        .map_err(|e| ApiError::Internal(Box::new(std::io::Error::other(e.to_string()))))
}

pub async fn get_target(pool: &PgPool, id: Uuid) -> Result<BackupTargetRow, ApiError> {
    sqlx::query_as::<_, BackupTargetRow>("SELECT * FROM backup_targets WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(ApiError::from)
        .map_err(|e| ApiError::Internal(Box::new(std::io::Error::other(e.to_string()))))?
        .ok_or(ApiError::NotFound("target not found".into()))
}

/// Probes a target row (connectivity + snapshot count + fingerprint).
pub async fn probe_row(row: &BackupTargetRow) -> Result<ProbeOutcome, ApiError> {
    let conn = row.connect().await?;
    Ok(conn.probe(&row.prefix).await)
}

/// Identity dedup on save: rejects a second config describing the same
/// physical store (fingerprint equality ignoring the row being edited).
pub async fn assert_identity_unique(
    pool: &PgPool,
    candidate: &BackupTargetRow,
    ignore_id: Option<Uuid>,
) -> Result<(), ApiError> {
    let cand_fp = candidate.config_fingerprint()?;
    if cand_fp.0.is_empty() {
        return Ok(()); // WebDAV before first claim: identity unknown yet
    }
    for other in list_targets(pool).await? {
        if Some(other.id) == ignore_id {
            continue;
        }
        if other.kind != candidate.kind {
            continue;
        }
        if let Ok(fp) = other.config_fingerprint()
            && !fp.0.is_empty()
            && fp == cand_fp
        {
            return Err(ApiError::Conflict(format!(
                "target '{}' already describes this storage",
                other.name
            )));
        }
    }
    Ok(())
}

/// deleteCheck: only provably-emptied targets (or unreachable ones whose
/// fingerprint-identical sibling is reachable and emptied) may be deleted.
pub async fn delete_check(pool: &PgPool, id: Uuid) -> Result<(), ApiError> {
    let row = get_target(pool, id).await?;
    let probe = probe_row(&row).await?;
    if probe.reachable {
        if probe.snapshot_objects > 0 {
            return Err(ApiError::Conflict(format!(
                "target '{}' still holds {} snapshot object(s); empty it first",
                row.name, probe.snapshot_objects
            )));
        }
        return Ok(());
    }
    // Unreachable: accept only when a sibling with the same fingerprint is
    // reachable AND provably empty (duplicate config of one storage).
    let fp = row.config_fingerprint()?;
    for other in list_targets(pool).await? {
        if other.id == row.id || other.kind != row.kind {
            continue;
        }
        if let Ok(ofp) = other.config_fingerprint()
            && ofp == fp
            && let Ok(op) = probe_row(&other).await
            && op.reachable
            && op.snapshot_objects == 0
        {
            return Ok(());
        }
    }
    Err(ApiError::Conflict(format!(
        "target '{}' is unreachable and cannot be proven empty",
        row.name
    )))
}

/// editCheck: connection-field changes need proof; display/retention edits
/// skip probing entirely.
pub async fn edit_check(
    _pool: &PgPool,
    old: &BackupTargetRow,
    new_conn: &BackupTargetRow,
) -> Result<(), ApiError> {
    let connection_changed = old.kind != new_conn.kind
        || old.endpoint != new_conn.endpoint
        || old.bucket != new_conn.bucket
        || old.prefix != new_conn.prefix
        || old.access_key != new_conn.access_key
        || old.secure != new_conn.secure;
    if !connection_changed {
        return Ok(());
    }
    // Same physical storage (new config fingerprints equal) → allowed.
    let old_fp = old.config_fingerprint()?;
    let new_fp = new_conn.config_fingerprint()?;
    if !old_fp.0.is_empty() && old_fp == new_fp {
        return Ok(());
    }
    // Otherwise the old storage must be provably emptied.
    match probe_row(old).await {
        Ok(p) if p.reachable && p.snapshot_objects == 0 => Ok(()),
        Ok(p) if p.reachable => Err(ApiError::Conflict(format!(
            "old storage still holds {} snapshot object(s); empty it before changing connection fields",
            p.snapshot_objects
        ))),
        _ => Err(ApiError::Conflict(
            "old storage is unreachable and cannot be proven empty".into(),
        )),
    }
}

/// upsert_target writes a validated target row (secrets already resolved).
pub async fn upsert_target(
    pool: &PgPool,
    id: Option<Uuid>,
    input: &TargetInput,
) -> Result<BackupTargetRow, ApiError> {
    if input.users.is_empty() && input.enabled {
        return Err(ApiError::BadRequest(
            "a target with no assigned users cannot be enabled".into(),
        ));
    }
    if !matches!(input.kind.as_str(), "s3" | "webdav" | "pg") {
        return Err(ApiError::BadRequest("kind must be s3|webdav|pg".into()));
    }
    let now = crate::auth::service::now_ms();
    match id {
        Some(id) => {
            let old = get_target(pool, id).await?;
            let mut candidate = row_from_input(input, now, Some(old.id));
            candidate.access_key = resolve_secret(&input.access_key, &old.access_key);
            candidate.secret_key = resolve_secret(&input.secret_key, &old.secret_key);
            edit_check(pool, &old, &candidate).await?;
            assert_identity_unique(pool, &candidate, Some(id)).await?;
            sqlx::query(
                "UPDATE backup_targets SET kind=$2, name=$3, endpoint=$4, bucket=$5, prefix=$6, \
                 access_key=$7, secret_key=$8, secure=$9, region=$10, provider=$11, \
                 keep_last=$12, max_age_days=$13, enabled=$14, users=$15, updated_at=$16 \
                 WHERE id = $1",
            )
            .bind(id)
            .bind(&candidate.kind)
            .bind(&candidate.name)
            .bind(&candidate.endpoint)
            .bind(&candidate.bucket)
            .bind(&candidate.prefix)
            .bind(&candidate.access_key)
            .bind(&candidate.secret_key)
            .bind(candidate.secure)
            .bind(&candidate.region)
            .bind(&candidate.provider)
            .bind(candidate.keep_last)
            .bind(candidate.max_age_days)
            .bind(candidate.enabled)
            .bind(&candidate.users)
            .bind(now)
            .execute(pool)
            .await
            .map_err(ApiError::from)?;
            get_target(pool, id).await
        }
        None => {
            let mut candidate = row_from_input(input, now, None);
            if candidate.access_key == MASK || candidate.secret_key == MASK {
                return Err(ApiError::BadRequest(
                    "new target cannot use *** as a credential".into(),
                ));
            }
            assert_identity_unique(pool, &candidate, None).await?;
            let id = Uuid::new_v4();
            candidate.id = id;
            sqlx::query(
                "INSERT INTO backup_targets (id, kind, name, endpoint, bucket, prefix, access_key, \
                 secret_key, secure, region, provider, keep_last, max_age_days, enabled, users, \
                 last_result, created_at, updated_at) \
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,'',$16,$16)",
            )
            .bind(candidate.id)
            .bind(&candidate.kind)
            .bind(&candidate.name)
            .bind(&candidate.endpoint)
            .bind(&candidate.bucket)
            .bind(&candidate.prefix)
            .bind(&candidate.access_key)
            .bind(&candidate.secret_key)
            .bind(candidate.secure)
            .bind(&candidate.region)
            .bind(&candidate.provider)
            .bind(candidate.keep_last)
            .bind(candidate.max_age_days)
            .bind(candidate.enabled)
            .bind(&candidate.users)
            .bind(now)
            .execute(pool)
            .await
            .map_err(ApiError::from)?;
            get_target(pool, id).await
        }
    }
}

fn row_from_input(input: &TargetInput, now: i64, id: Option<Uuid>) -> BackupTargetRow {
    BackupTargetRow {
        id: id.unwrap_or_else(Uuid::nil),
        kind: input.kind.clone(),
        name: input.name.clone(),
        endpoint: input.endpoint.clone(),
        bucket: input.bucket.clone(),
        prefix: input.prefix.clone(),
        access_key: input.access_key.clone(),
        secret_key: input.secret_key.clone(),
        secure: input.secure,
        region: input.region.clone(),
        provider: input.provider.clone(),
        keep_last: input.keep_last,
        max_age_days: input.max_age_days,
        enabled: input.enabled,
        users: input.users.clone(),
        last_result: String::new(),
        created_at: now,
        updated_at: now,
    }
}

fn resolve_secret(incoming: &str, previous: &str) -> String {
    if incoming == MASK {
        previous.to_string()
    } else {
        incoming.to_string()
    }
}

/// Empties a target: deletes every snapshot object (manifests+blobs+refs).
/// Returns the number of deleted objects.
pub async fn empty_target(pool: &PgPool, id: Uuid) -> Result<usize, ApiError> {
    let row = get_target(pool, id).await?;
    let conn = row.connect().await?;
    let objects = conn.list(&row.prefix).await?;
    let mut deleted = 0usize;
    for (key, _, _) in objects {
        if conn.delete(&key).await.is_ok() {
            deleted += 1;
        }
    }
    Ok(deleted)
}
