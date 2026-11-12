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
    /// Standard 5-field cron; empty = manual only.
    #[serde(default)]
    pub schedule: String,
    #[serde(default)]
    pub enabled: bool,
    /// Owning user ids (empty = unassigned, cannot enable).
    #[serde(default)]
    pub users: Vec<Uuid>,
    /// Namespace is server-allocated (ULID) at creation and never accepted
    /// from the client — two instances sharing a store stay isolated without
    /// anyone configuring it. Ignored when supplied.
    #[serde(default, skip_serializing)]
    pub namespace: String,
    /// Present when the client ran "test connection" on these connection
    /// fields. The server recomputes the token and rejects the save when the
    /// tested fields differ from the submitted ones (siyuan #15 §9).
    #[serde(default, skip_serializing)]
    pub connection_token: Option<String>,
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
        "schedule": row.schedule,
        "enabled": row.enabled,
        "last_result": row.last_result,
        "users": row.users,
        "namespace": row.namespace,
        "created_at": row.created_at,
        "updated_at": row.updated_at,
    })
}

// The row carries `users` now; keep it in the struct via a thin alias so the
// sqlx FromRow derive in config.rs stays in one place.
/// Backfills a namespace for rows created before the column existed. Called
/// on every list so a deployment upgrading from an older build self-heals
/// instead of writing those targets' objects into the shared root scope.
async fn backfill_namespaces(pool: &PgPool) -> Result<(), ApiError> {
    let rows: Vec<(Uuid,)> = sqlx::query_as("SELECT id FROM backup_targets WHERE namespace = ''")
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    for (id,) in rows {
        let _ = sqlx::query("UPDATE backup_targets SET namespace = $2 WHERE id = $1")
            .bind(id)
            .bind(crate::target::owner::new_namespace_id())
            .execute(pool)
            .await;
    }
    Ok(())
}

pub async fn list_targets(pool: &PgPool) -> Result<Vec<BackupTargetRow>, ApiError> {
    let _ = backfill_namespaces(pool).await;
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
    // Siyuan fork §1 parity: only S3 and WebDAV. Stored `pg` rows from older
    // builds are inert: kind() rejects them, so they can be listed and
    // deleted but never connected.
    if !matches!(input.kind.as_str(), "s3" | "webdav") {
        return Err(ApiError::BadRequest("kind must be s3|webdav".into()));
    }
    // Save gate (siyuan #15 §9): a target may only be written after its
    // connection fields were tested. Existing targets may skip the gate when
    // the connection is unchanged — retention/name/schedule edits are safe.
    if let Some(token) = input.connection_token.as_deref() {
        let expected = connection_token(input);
        if token != expected {
            return Err(ApiError::Conflict(
                "connection fields changed since the test; run test connection again".into(),
            ));
        }
    } else {
        let connection_needs_test = match id {
            None => true,
            Some(id) => {
                let old = get_target(pool, id).await?;
                let old_input = input_from_row(&old);
                connection_token(&old_input) != connection_token(input)
            }
        };
        if connection_needs_test {
            return Err(ApiError::Conflict(
                "test the connection before saving this target".into(),
            ));
        }
    }
    if !input.schedule.trim().is_empty() {
        crate::target::cron::CronSpec::parse(input.schedule.trim())
            .map_err(|e| ApiError::BadRequest(format!("invalid schedule: {e}")))?;
    }
    let now = crate::auth::service::now_ms();
    match id {
        Some(id) => {
            let old = get_target(pool, id).await?;
            let mut candidate = row_from_input(input, now, Some(old.id));
            candidate.access_key = resolve_secret(&input.access_key, &old.access_key);
            candidate.secret_key = resolve_secret(&input.secret_key, &old.secret_key);
            // Namespace is server-owned and immutable: it names the object
            // scope, so a change would orphan everything already written.
            candidate.namespace = old.namespace.clone();
            edit_check(pool, &old, &candidate).await?;
            assert_identity_unique(pool, &candidate, Some(id)).await?;
            sqlx::query(
                "UPDATE backup_targets SET kind=$2, name=$3, endpoint=$4, bucket=$5, prefix=$6, \
                 access_key=$7, secret_key=$8, secure=$9, region=$10, provider=$11, \
                 keep_last=$12, max_age_days=$13, enabled=$14, users=$15, schedule=$16, \
                 namespace=$17, updated_at=$18 WHERE id = $1",
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
            .bind(&candidate.schedule)
            .bind(&candidate.namespace)
            .bind(now)
            .execute(pool)
            .await
            .map_err(ApiError::from)?;
            get_target(pool, id).await
        }
        None => {
            let mut candidate = row_from_input(input, now, None);
            // Auto-allocated scope id (siyuan #15 §1: ULID, never user-typed).
            candidate.namespace = crate::target::owner::new_namespace_id();
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
                 schedule, last_result, created_at, updated_at, namespace) \
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,'',$17,$17,$18)",
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
            .bind(&candidate.schedule)
            .bind(&candidate.namespace)
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
        schedule: input.schedule.clone(),
        enabled: input.enabled,
        users: input.users.clone(),
        // Assigned by the caller (ULID on create, stored value on update).
        namespace: String::new(),
        last_result: String::new(),
        created_at: now,
        updated_at: now,
    }
}

/// Hash of the connection *identity* fields (no secrets), used by the UI to
/// bind a successful probe to the exact connection it validated (siyuan #15
/// §9). Display/retention/schedule fields are deliberately excluded: editing
/// a name must not force a re-test.
pub fn connection_token(input: &TargetInput) -> String {
    use sha2::{Digest, Sha256};
    let material = format!(
        "{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
        input.kind.trim().to_lowercase(),
        input.endpoint.trim().trim_end_matches('/').to_lowercase(),
        input.bucket.trim().to_lowercase(),
        input.prefix.trim().trim_matches('/').to_lowercase(),
        input.region.trim().to_lowercase(),
        input.provider.trim().to_lowercase(),
        input.secure,
    );
    let digest = Sha256::digest(material.as_bytes());
    digest.iter().take(12).map(|b| format!("{b:02x}")).collect()
}

/// Reconstructs the connection-bearing part of an input from a stored row,
/// so token comparison does not need the client to resend anything.
fn input_from_row(row: &BackupTargetRow) -> TargetInput {
    TargetInput {
        kind: row.kind.clone(),
        name: row.name.clone(),
        endpoint: row.endpoint.clone(),
        bucket: row.bucket.clone(),
        prefix: row.prefix.clone(),
        access_key: row.access_key.clone(),
        secret_key: row.secret_key.clone(),
        secure: row.secure,
        region: row.region.clone(),
        provider: row.provider.clone(),
        keep_last: row.keep_last,
        max_age_days: row.max_age_days,
        schedule: row.schedule.clone(),
        enabled: row.enabled,
        users: row.users.clone(),
        namespace: String::new(), // server-owned, never part of the token
        connection_token: None,
    }
}

/// Public wrapper for the draft-probe route (secret rehydration).
pub fn resolve_secret_pub(incoming: &str, previous: &str) -> String {
    resolve_secret(incoming, previous)
}

/// Public wrapper for the draft-probe route (build a candidate row).
pub fn row_from_input_pub(input: &TargetInput, now: i64, id: Option<Uuid>) -> BackupTargetRow {
    row_from_input(input, now, id)
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
    let objects = conn.list(conn.list_prefix()).await?;
    let mut deleted = 0usize;
    for (key, _, _) in objects {
        if conn.delete(&key).await.is_ok() {
            deleted += 1;
        }
    }
    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(over: impl FnOnce(&mut TargetInput)) -> TargetInput {
        let mut t = TargetInput {
            kind: "s3".into(),
            name: "t".into(),
            endpoint: "https://s3.example.com".into(),
            bucket: "bk".into(),
            prefix: "pergame".into(),
            access_key: "AK".into(),
            secret_key: "SK".into(),
            secure: true,
            region: "us-east-1".into(),
            provider: "Other".into(),
            keep_last: 10,
            max_age_days: 0,
            schedule: String::new(),
            enabled: true,
            users: vec![Uuid::nil()],
            namespace: String::new(),
            connection_token: None,
        };
        over(&mut t);
        t
    }

    #[test]
    fn connection_token_ignores_display_and_retention() {
        let base = connection_token(&input(|_| {}));
        // Name / retention / schedule are not connection identity.
        let display_only = connection_token(&input(|t| {
            t.name = "renamed".into();
            t.keep_last = 3;
            t.schedule = "0 3 * * *".into();
            t.users = vec![Uuid::nil(), Uuid::max()];
        }));
        assert_eq!(base, display_only);
        // Connection fields do change it.
        let moved = connection_token(&input(|t| t.endpoint = "https://other.example".into()));
        assert_ne!(base, moved);
    }

    #[test]
    fn connection_token_never_leaks_secrets() {
        let token = connection_token(&input(|t| {
            t.access_key = "SUPER-SECRET-AK".into();
            t.secret_key = "SUPER-SECRET-SK".into();
        }));
        assert!(!token.contains("SUPER"));
        assert_eq!(token.len(), 24, "12 bytes hex");
    }

    #[test]
    fn namespace_is_server_allocated_ulid() {
        use crate::target::owner::new_namespace_id;
        let a = new_namespace_id();
        let b = new_namespace_id();
        assert_eq!(a.len(), 26, "ULID is 26 Crockford Base32 chars");
        assert_eq!(b.len(), 26);
        assert_ne!(a, b, "each target gets its own scope");
        for id in [&a, &b] {
            assert!(
                id.bytes().all(|c| {
                    c.is_ascii_digit()
                        || (c.is_ascii_uppercase()
                            && c != b'I'
                            && c != b'L'
                            && c != b'O'
                            && c != b'U')
                }),
                "Crockford Base32 alphabet only: {id}"
            );
        }
        // Time-ordered prefix: two ids generated in sequence share the high
        // chars far more often than random ones would.
        assert_eq!(a[..6], b[..6], "timestamp prefix should be close");
    }

    #[test]
    fn client_supplied_namespace_is_ignored() {
        // The input carries the field only so old clients don't break; the
        // row builder drops it and the caller assigns a ULID instead.
        let row = row_from_input(&input(|t| t.namespace = "user-typed".into()), 0, None);
        assert_eq!(row.namespace, "", "client value must not reach the row");
    }
}
