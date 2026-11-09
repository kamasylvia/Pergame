//! /backup routes (Go backup.controller.go parity): create (202 + job_id),
//! status, cancel.

use axum::Json;
use axum::extract::{Multipart, Path, State};
use serde::Deserialize;
use serde_json::json;

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
    let options = options_from(body.map(|b| b.0).unwrap_or_default());

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
            crate::backup::run_backup(store, sql, storage, user_id, spawn_job_id, options).await;
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

impl BackupStore {
    /// Placeholder path parity: Go fails fast when the storage client is nil.
    pub async fn run_backup_no_storage(&self, user_id: i64, job_id: String) {
        let _ = user_id;
        self.update(&job_id, |j| {
            j.progress = 0;
            j.message = "minio client not initialized".into();
            j.error = Some("minio client not initialized".into());
        })
        .await;
    }
}

// ---------------- snapshots + restore (spec backup-restore.md) ----------------

fn snapshot_key(user_id: i64, id: &str) -> Result<String, ApiError> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
        return Err(ApiError::BadRequest("invalid snapshot id".into()));
    }
    Ok(format!("{user_id}/snapshots/{id}.zip"))
}

/// GET /backup/snapshots — caller's snapshots, newest first.
pub async fn list_snapshots(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let storage = state
        .storage
        .clone()
        .ok_or(ApiError::NotFound("storage not configured".into()))?;
    let mut snaps = storage.list_backup_snapshots(auth.user_id()).await?;
    snaps.sort_by(|a, b| b.0.cmp(&a.0));
    let mut rows = Vec::new();
    for (key, size, mtime) in snaps {
        let Some(id) = key
            .rsplit('/')
            .next()
            .and_then(|f| f.strip_suffix(".zip"))
            .map(|s| s.to_string())
        else {
            continue;
        };
        // Sidecar memo (older snapshots may not have one).
        let memo = storage
            .get_backup_object(&format!("{}/snapshots/{id}.json", auth.user_id()))
            .await
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
            .and_then(|v| {
                v.get("memo")
                    .and_then(|m| m.as_str())
                    .map(|s| s.to_string())
            });
        rows.push(json!({
            "id": id,
            "size": size,
            "last_modified": mtime,
            "memo": memo,
        }));
    }
    Ok(ApiOk::ok(json!(rows)))
}

/// GET /backup/snapshots/:id/download — presigned URL (24h).
pub async fn download_snapshot(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let storage = state
        .storage
        .clone()
        .ok_or(ApiError::NotFound("storage not configured".into()))?;
    let key = snapshot_key(auth.user_id(), &id)?;
    let filename = format!("pergame-snapshot-{id}.zip");
    let url = storage
        .presign_backup_get(&key, &filename)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "presign snapshot failed");
            ApiError::Internal(Box::new(std::io::Error::other(
                "failed to sign snapshot URL",
            )))
        })?;
    Ok(ApiOk::ok(json!({ "download_url": url })))
}

/// DELETE /backup/snapshots/:id
pub async fn delete_snapshot(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let storage = state
        .storage
        .clone()
        .ok_or(ApiError::NotFound("storage not configured".into()))?;
    let key = snapshot_key(auth.user_id(), &id)?;
    storage.delete_backup_object(&key).await?;
    Ok(ApiOk::ok(json!("Snapshot deleted successfully.")))
}

/// Safety snapshot (SiYuan "Backup before checkout" parity): taken before any
/// restore so the replaced state stays recoverable. Memo marks it. Returns the
/// snapshot id, appended to the restore report's warnings.
async fn safety_snapshot(state: &AppState, user_id: i64) -> Option<String> {
    let storage = state.storage.clone()?;
    let options = crate::backup::BackupOptions {
        include_documents: true,
        include_settings: true,
        memo: Some("Pre-restore safety snapshot".into()),
        ..Default::default()
    };
    let job_id = state.backups.create(user_id, options).await.ok()?;
    // Synchronous wait: snapshots are small (documents-only), bounded at ~30s.
    for _ in 0..60 {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        if let Ok(job) = state.backups.get(user_id, &job_id).await {
            if job.error.is_some() {
                tracing::warn!(job = %job_id, "safety snapshot failed");
                return None;
            }
            if job.download_url.is_some() {
                let snaps = storage.list_backup_snapshots(user_id).await.ok()?;
                let latest = snaps.iter().map(|(k, _, _)| k).max()?;
                return latest
                    .rsplit('/')
                    .next()
                    .and_then(|f| f.strip_suffix(".zip"))
                    .map(|s| s.to_string());
            }
        }
    }
    tracing::warn!("safety snapshot timed out");
    None
}

/// POST /backup/snapshots/:id/restore — replace current data with the snapshot.
pub async fn restore_snapshot(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<ApiOk<crate::restore::RestoreReport>, ApiError> {
    let storage = state
        .storage
        .clone()
        .ok_or(ApiError::NotFound("storage not configured".into()))?;
    let key = snapshot_key(auth.user_id(), &id)?;
    let bytes = storage.get_backup_object(&key).await?;
    let mut report = crate::restore::restore_from_zip(
        state.db.clone(),
        Some(storage.clone()),
        auth.user_id(),
        bytes,
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
/// (multipart field "file"; ≤256MB body limit on this route).
pub async fn restore_upload(
    State(state): State<AppState>,
    auth: AuthUser,
    mut multipart: Multipart,
) -> Result<ApiOk<crate::restore::RestoreReport>, ApiError> {
    let mut bytes: Option<Vec<u8>> = None;
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
            bytes = Some(data.to_vec());
            break;
        }
    }
    let bytes = bytes.ok_or(ApiError::BadRequest(
        "multipart field \"file\" is required".into(),
    ))?;
    let report = crate::restore::restore_from_zip(
        state.db.clone(),
        state.storage.clone(),
        auth.user_id(),
        bytes,
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
) -> Result<ApiOk<crate::restore::RestorePlan>, ApiError> {
    let storage = state
        .storage
        .clone()
        .ok_or(ApiError::NotFound("storage not configured".into()))?;
    let key = snapshot_key(auth.user_id(), &id)?;
    let bytes = storage.get_backup_object(&key).await?;
    let plan = crate::restore::plan_from_zip(state.db.clone(), auth.user_id(), bytes).await?;
    Ok(ApiOk::ok(plan))
}

/// POST /backup/restore/upload/preview — dry-run plan for an uploaded zip.
pub async fn preview_upload(
    State(state): State<AppState>,
    auth: AuthUser,
    mut multipart: Multipart,
) -> Result<ApiOk<crate::restore::RestorePlan>, ApiError> {
    let bytes = read_upload(&mut multipart).await?;
    let plan = crate::restore::plan_from_zip(state.db.clone(), auth.user_id(), bytes).await?;
    Ok(ApiOk::ok(plan))
}
