//! /backup routes (Go backup.controller.go parity): create (202 + job_id),
//! status, cancel.

use axum::Json;
use axum::extract::{Multipart, Path, State};
use axum::response::IntoResponse;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::backup::{BackupOptions, BackupStore};
use crate::error::ApiError;
use crate::response::ApiOk;

#[derive(Debug, Deserialize, Default)]
pub struct BackupBody {
    #[serde(default)]
    pub include_documents: bool,
    #[serde(default)]
    pub include_files: bool,
    #[serde(default)]
    pub include_metadata: bool,
    #[serde(default)]
    pub include_settings: bool,
    #[serde(default)]
    pub memo: Option<String>,
    /// Snapshot password (fork §2 semantics). Some = encrypt with it, None =
    /// plain snapshot. Never stored; the caller must supply it again to
    /// restore or download.
    #[serde(default)]
    pub passphrase: Option<String>,
    /// Zero-target zip lifetime: "1h" | "1d" | "1w" (default "1h").
    /// Unknown values fall back to the default.
    #[serde(default)]
    pub expires_in: Option<String>,
}

fn options_from(body: BackupBody) -> BackupOptions {
    BackupOptions {
        include_documents: body.include_documents,
        include_files: body.include_files,
        include_metadata: body.include_metadata,
        include_settings: body.include_settings,
        memo: body
            .memo
            .filter(|m| !m.trim().is_empty())
            .map(|m| m.trim().chars().take(200).collect()),
    }
}

/// POST /backup — starts an async backup job; 202 {job_id}.
pub async fn create_backup(
    State(state): State<AppState>,
    auth: AuthUser,
    body: Option<Json<BackupBody>>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let body = body.map(|b| b.0).unwrap_or_default();
    let passphrase = body.passphrase.clone();
    let ttl_ms = match body.expires_in.as_deref().unwrap_or("1h") {
        "1d" => 24 * 3600 * 1000,
        "1w" => 7 * 24 * 3600 * 1000,
        _ => crate::backup::INLINE_ZIP_TTL_MS, // "1h" + unknown
    };
    let options = options_from(body);

    let job_id = state
        .backups
        .create(auth.user_id(), options.clone())
        .await?;
    let job_id_for_response = job_id.clone();

    // Spawn the runner with owned handles (storage optional → runtime error
    // path inside the job like Go's minio-nil check).
    let store = state.backups.clone();
    let sql = state.db.clone();
    let user_id = auth.user_id();
    let spawn_job_id = job_id.clone();
    if let Some(storage) = state.storage.clone() {
        tokio::spawn(async move {
            crate::backup::run_backup(
                store,
                sql,
                storage,
                user_id,
                spawn_job_id,
                options,
                passphrase,
                ttl_ms,
            )
            .await;
        });
    } else {
        tokio::spawn(async move {
            crate::backup::run_backup_no_storage(store, user_id, spawn_job_id).await;
        });
    }
    Ok(ApiOk::with_status(
        axum::http::StatusCode::ACCEPTED,
        json!({ "job_id": job_id_for_response }),
    ))
}

/// GET /backup/:jobId
pub async fn get_backup_status(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(job_id): Path<String>,
) -> Result<ApiOk<crate::backup::BackupJob>, ApiError> {
    if job_id.is_empty() {
        return Err(ApiError::BadRequest("job ID is required".into()));
    }
    let job = state.backups.get(auth.user_id(), &job_id).await?;
    Ok(ApiOk::ok(job))
}

/// DELETE /backup/:jobId — cancel an ongoing job.
pub async fn cancel_backup(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(job_id): Path<String>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    if job_id.is_empty() {
        return Err(ApiError::BadRequest("job ID is required".into()));
    }
    state.backups.cancel(auth.user_id(), &job_id).await?;
    Ok(ApiOk::ok(
        json!({ "message": "Backup job cancelled successfully" }),
    ))
}

/// GET /backup/:jobId/download?token=... — one-shot download of a zero-target
/// snapshot zip (fork §4 "save zip locally"). The bytes are removed on
/// first successful read; re-download requires a fresh snapshot.
/// Authorized by the token alone (no session needed): links stay valid
/// across access-token expiry. The user id comes from the session when
/// present, else from the job id (`user-{uid}-*`); a mismatched pair
/// simply misses the sidecar and 404s.
pub async fn download_job_zip(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
    Path(job_id): Path<String>,
    axum::extract::Query(query): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<axum::response::Response, ApiError> {
    use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
    use axum::response::IntoResponse;
    if job_id.is_empty() {
        return Err(ApiError::BadRequest("job ID is required".into()));
    }
    let token = query.get("token").map(|s| s.as_str()).unwrap_or("");
    if token.is_empty() {
        return Err(ApiError::NotFound(
            "snapshot zip expired or already downloaded".into(),
        ));
    }
    let user_id = match auth {
        Some(a) => a.user_id(),
        None => job_id
            .strip_prefix("user-")
            .and_then(|s| s.rsplit_once('-'))
            .and_then(|(uid, _)| uuid::Uuid::parse_str(uid).ok())
            .ok_or(ApiError::NotFound(
                "snapshot zip expired or already downloaded".into(),
            ))?,
    };
    let storage = state
        .storage
        .clone()
        .ok_or(ApiError::NotFound("storage not configured".into()))?;
    let bytes = state
        .backups
        .take_inline_zip(&storage, user_id, &job_id, token)
        .await?;
    let disposition = format!("attachment; filename=\"pergame-snapshot-{job_id}.zip\"");
    Ok((
        axum::http::StatusCode::OK,
        [
            (CONTENT_TYPE, "application/zip"),
            (CONTENT_DISPOSITION, disposition.as_str()),
        ],
        bytes,
    )
        .into_response())
}

impl BackupStore {
    /// Placeholder path parity: Go fails fast when the storage client is nil.
    pub async fn run_backup_no_storage(&self, user_id: Uuid, job_id: String) {
        let _ = user_id;
        self.update(&job_id, |j| {
            j.progress = 0;
            j.message = "minio client not initialized".into();
            j.error = Some("minio client not initialized".into());
        })
        .await;
    }
}

/// Restore request body: passphrase unlocks encrypted snapshots; force
/// confirms overwriting a non-empty workspace (fork §5: preview first).
#[derive(Debug, Deserialize, Default)]
pub struct PassphraseBody {
    #[serde(default)]
    pub passphrase: Option<String>,
    #[serde(default)]
    pub force: bool,
}

/// Loads a snapshot as restorable zip bytes from the first reachable target,
/// assembling the v2 manifest + blobs into the v1 shape the restore engine
/// consumes. `passphrase`: snapshot password for encrypted manifests (fork
/// §2: the caller supplies it; the server never stores it). Legacy service-
/// bucket zips are not read back (no read fallback by design).
async fn load_snapshot_bytes(
    db: &sqlx::PgPool,
    user_id: Uuid,
    id: &str,
    passphrase: Option<&str>,
) -> Result<Vec<u8>, ApiError> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
        return Err(ApiError::BadRequest("invalid snapshot id".into()));
    }
    let conn = crate::target::fanout::first_readable_target(db, user_id)
        .await
        .ok_or(ApiError::NotFound(
            "snapshot not available (no reachable targets)".into(),
        ))?;
    let manifest_key = format!("{user_id}/manifests/{id}.json");
    let bytes = conn
        .get(&manifest_key)
        .await
        .map_err(|_| ApiError::NotFound("snapshot not found on target".into()))?;
    let manifest: crate::backup::BackupManifest = serde_json::from_slice(&bytes)
        .map_err(|e| ApiError::BadRequest(format!("manifest unreadable: {e}")))?;
    if manifest.user_id != user_id.to_string() {
        return Err(ApiError::Forbidden("not your snapshot".into()));
    }
    crate::restore::assemble_snapshot_zip(&conn, manifest, passphrase).await
}

/// GET /backup/snapshots — caller's snapshots, newest first.
/// Reads the first reachable target (all targets hold identical snapshots).
/// No targets or all unreachable = empty list: snapshots are invisible
/// rather than partially visible.
pub async fn list_snapshots(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let Some(conn) = crate::target::fanout::first_readable_target(&state.db, auth.user_id()).await
    else {
        return Ok(ApiOk::ok(json!([])));
    };
    let prefix = format!("{}/manifests/", auth.user_id());
    let objects = conn.list(&prefix).await.unwrap_or_default();
    let mut rows = Vec::new();
    for (key, _size, mtime) in objects {
        if !key.contains("/manifests/") {
            continue;
        }
        let Some(id) = key
            .rsplit('/')
            .next()
            .and_then(|f| f.strip_suffix(".json"))
            .map(|s| s.to_string())
        else {
            continue;
        };
        let meta = conn.get(&key).await.ok().and_then(|b| {
            serde_json::from_slice::<serde_json::Value>(&b)
                .ok()
                .map(|v| {
                    (
                        v.get("memo")
                            .and_then(|m| m.as_str())
                            .map(|s| s.to_string()),
                        v.get("logical_bytes").and_then(|n| n.as_u64()).unwrap_or(0),
                        v.get("encryption").is_some(),
                    )
                })
        });
        let (memo, size, encrypted) = meta.unwrap_or((None, 0, false));
        rows.push(json!({
            "id": id,
            "size": size,
            "last_modified": mtime,
            "memo": memo,
            "format": "v2",
            "encrypted": encrypted,
        }));
    }
    rows.sort_by_key(|r| -r.get("last_modified").and_then(|v| v.as_i64()).unwrap_or(0));
    Ok(ApiOk::ok(json!(rows)))
}

/// GET /backup/snapshots/:id/download — streams the snapshot zip directly
/// (cookie-authenticated; assembled from the first reachable target).
/// Portable export keeps ciphertext blobs as-is (fork §7): no passphrase
/// needed for download, only for restore.
pub async fn download_snapshot(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<axum::response::Response, ApiError> {
    use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
        return Err(ApiError::BadRequest("invalid snapshot id".into()));
    }
    let conn = crate::target::fanout::first_readable_target(&state.db, auth.user_id())
        .await
        .ok_or(ApiError::NotFound(
            "snapshot not available (no reachable targets)".into(),
        ))?;
    let manifest_key = format!("{}/manifests/{id}.json", auth.user_id());
    let raw = conn
        .get(&manifest_key)
        .await
        .map_err(|_| ApiError::NotFound("snapshot not found on target".into()))?;
    let manifest: crate::backup::BackupManifest = serde_json::from_slice(&raw)
        .map_err(|e| ApiError::BadRequest(format!("manifest unreadable: {e}")))?;
    if manifest.user_id != auth.user_id().to_string() {
        return Err(ApiError::Forbidden("not your snapshot".into()));
    }
    let bytes = crate::restore::export_portable_zip_from_conn(&conn, &manifest).await?;
    let filename = format!("pergame-snapshot-{id}.zip");
    let disposition = format!("attachment; filename=\"{filename}\"");
    Ok((
        axum::http::StatusCode::OK,
        [
            (CONTENT_TYPE, "application/zip"),
            (CONTENT_DISPOSITION, disposition.as_str()),
        ],
        bytes,
    )
        .into_response())
}

/// DELETE /backup/snapshots/:id — deletes the manifest from every reachable
/// target (blobs are GC'd by each target's retention pass). Unreachable
/// targets are reported, not silently skipped.
pub async fn delete_snapshot(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    if crate::locks::global().is_restoring().await {
        return Err(ApiError::Conflict(
            "a restore is running; snapshot deletion must wait".into(),
        ));
    }
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
        return Err(ApiError::BadRequest("invalid snapshot id".into()));
    }
    let targets = crate::target::fanout::readable_targets(&state.db, auth.user_id()).await;
    if targets.is_empty() {
        return Err(ApiError::NotFound(
            "snapshot not available (no reachable targets)".into(),
        ));
    }
    let manifest_key = format!("{}/manifests/{id}.json", auth.user_id());
    let mut deleted = 0usize;
    let mut missing = Vec::new();
    for (name, conn) in &targets {
        match conn.delete(&manifest_key).await {
            Ok(true) => deleted += 1,
            _ => missing.push(name.clone()),
        }
    }
    if deleted == 0 {
        return Err(ApiError::NotFound(
            "snapshot not found on any target".into(),
        ));
    }
    if missing.is_empty() {
        Ok(ApiOk::ok(json!("Snapshot deleted successfully.")))
    } else {
        Ok(ApiOk::ok(json!(format!(
            "Snapshot deleted on {deleted} target(s); not found on: {}",
            missing.join(", ")
        ))))
    }
}

/// Safety snapshot (SiYuan "Backup before checkout" parity): taken before any
/// restore so the replaced state stays recoverable. Memo marks it. Returns the
/// snapshot id, appended to the restore report's warnings.
///
/// Runs inline (not via the job registry): the old code created a job row but
/// never spawned its runner, so it always timed out and no safety snapshot
/// was ever taken. Plain (no passphrase): it guards the restore itself, and
/// encrypting it would demand a second password at the worst moment.
async fn safety_snapshot(state: &AppState, user_id: Uuid) -> Option<String> {
    let storage = state.storage.clone()?;
    let options = crate::backup::BackupOptions {
        include_documents: true,
        include_settings: true,
        memo: Some("Pre-restore safety snapshot".into()),
        ..Default::default()
    };
    let job_id = format!("user-{user_id}-safety");
    state
        .backups
        .register(&job_id, user_id, options.clone())
        .await;
    crate::backup::run_backup(
        state.backups.clone(),
        state.db.clone(),
        storage.clone(),
        user_id,
        job_id,
        options,
        None,
        crate::backup::INLINE_ZIP_TTL_MS,
    )
    .await;
    // run_backup reports via the job row; confirm a fresh manifest landed on
    // the first readable target (zero-target safety snapshots stay in the
    // job's inline zip and are dropped with it — warn, don't fail).
    let conn = crate::target::fanout::first_readable_target(&state.db, user_id).await?;
    let prefix = format!("{user_id}/manifests/");
    let objects = conn.list(&prefix).await.ok()?;
    let latest = objects.iter().map(|(k, _, _)| k).max()?;
    latest
        .rsplit('/')
        .next()
        .and_then(|f| f.strip_suffix(".json"))
        .map(|s| s.to_string())
}

/// POST /backup/snapshots/:id/restore — replace current data with the snapshot.
pub async fn restore_snapshot(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    body: Option<Json<PassphraseBody>>,
) -> Result<ApiOk<crate::restore::RestoreReport>, ApiError> {
    let storage = state
        .storage
        .clone()
        .ok_or(ApiError::NotFound("storage not configured".into()))?;
    // Exclusive lock for the whole restore (blocks other backups/restores).
    let _guard = crate::locks::global()
        .acquire_restore()
        .await
        .map_err(|e| match e {
            crate::locks::LockError::RestoreInFlight => {
                ApiError::Conflict("another restore is already running".into())
            }
            crate::locks::LockError::BackupBusy => {
                ApiError::Conflict("a backup is still running; retry shortly".into())
            }
            crate::locks::LockError::TargetBusy(_) => {
                ApiError::Conflict("a backup is still running; retry shortly".into())
            }
        })?;
    let body = body.map(|b| b.0).unwrap_or_default();
    let bytes =
        load_snapshot_bytes(&state.db, auth.user_id(), &id, body.passphrase.as_deref()).await?;
    let mut report = crate::restore::restore_from_zip(
        state.db.clone(),
        Some(storage.clone()),
        auth.user_id(),
        bytes,
        body.force,
    )
    .await?;
    if let Some(safety_id) = safety_snapshot(&state, auth.user_id()).await {
        report
            .warnings
            .push(format!("pre-restore safety snapshot created: {safety_id}"));
    }
    Ok(ApiOk::ok(report))
}

/// POST /backup/restore/upload — restore from an uploaded backup zip
/// (multipart fields "file" + optional "force=true"; ≤256MB body limit).
pub async fn restore_upload(
    State(state): State<AppState>,
    auth: AuthUser,
    mut multipart: Multipart,
) -> Result<ApiOk<crate::restore::RestoreReport>, ApiError> {
    let mut bytes: Option<Vec<u8>> = None;
    let mut force = false;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::BadRequest(format!("multipart: {e}")))?
    {
        match field.name() {
            Some("file") => {
                let data = field
                    .bytes()
                    .await
                    .map_err(|e| ApiError::BadRequest(format!("file read: {e}")))?;
                bytes = Some(data.to_vec());
            }
            Some("force") => {
                force = field
                    .text()
                    .await
                    .map(|t| t.trim().eq_ignore_ascii_case("true"))
                    .unwrap_or(false);
            }
            _ => {}
        }
    }
    let mut bytes = bytes.ok_or(ApiError::BadRequest(
        "multipart field \"file\" is required".into(),
    ))?;
    if crate::restore::is_portable_zip(&bytes) {
        bytes = crate::restore::portable_to_v1_zip(bytes)?;
    }
    let _guard = crate::locks::global()
        .acquire_restore()
        .await
        .map_err(|e| match e {
            crate::locks::LockError::RestoreInFlight => {
                ApiError::Conflict("another restore is already running".into())
            }
            crate::locks::LockError::BackupBusy => {
                ApiError::Conflict("a backup is still running; retry shortly".into())
            }
            crate::locks::LockError::TargetBusy(_) => {
                ApiError::Conflict("a backup is still running; retry shortly".into())
            }
        })?;
    // Safety snapshot first (parity with snapshot restore).
    if let Some(safety_id) = safety_snapshot(&state, auth.user_id()).await {
        tracing::info!(safety = %safety_id, "safety snapshot taken before upload restore");
    }
    let report = crate::restore::restore_from_zip(
        state.db.clone(),
        state.storage.clone(),
        auth.user_id(),
        bytes,
        force,
    )
    .await?;
    Ok(ApiOk::ok(report))
}

// ---------------- restore preview (dry-run, spec backup-restore.md) ----------------

async fn read_upload(multipart: &mut Multipart) -> Result<Vec<u8>, ApiError> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::BadRequest(format!("multipart: {e}")))?
    {
        if field.name() == Some("file") {
            let data = field
                .bytes()
                .await
                .map_err(|e| ApiError::BadRequest(format!("file read: {e}")))?;
            return Ok(data.to_vec());
        }
    }
    Err(ApiError::BadRequest(
        "multipart field \"file\" is required".into(),
    ))
}

/// POST /backup/snapshots/:id/preview — dry-run plan against current data.
pub async fn preview_snapshot(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    body: Option<Json<PassphraseBody>>,
) -> Result<ApiOk<crate::restore::RestorePlan>, ApiError> {
    let passphrase = body.and_then(|b| b.0.passphrase);
    let bytes = load_snapshot_bytes(&state.db, auth.user_id(), &id, passphrase.as_deref()).await?;
    let plan = crate::restore::plan_from_zip(state.db.clone(), auth.user_id(), bytes).await?;
    Ok(ApiOk::ok(plan))
}

/// POST /backup/restore/upload/preview — dry-run plan for an uploaded zip.
pub async fn preview_upload(
    State(state): State<AppState>,
    auth: AuthUser,
    mut multipart: Multipart,
) -> Result<ApiOk<crate::restore::RestorePlan>, ApiError> {
    let mut bytes = read_upload(&mut multipart).await?;
    if crate::restore::is_portable_zip(&bytes) {
        bytes = crate::restore::portable_to_v1_zip(bytes)?;
    }
    let plan = crate::restore::plan_from_zip(state.db.clone(), auth.user_id(), bytes).await?;
    Ok(ApiOk::ok(plan))
}
