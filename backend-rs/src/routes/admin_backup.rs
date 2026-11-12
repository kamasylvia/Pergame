//! Admin backup-target management: instance-wide target CRUD for admins
//! (multi-user assignment, takeover). Regular users manage their own targets
//! via /api/backup/targets (routes::user_targets); both families share
//! target::repo.
//!
//! The dr-backup sidecar (pg_dump conf/jobs/cron) was removed 2026-11-12 and
//! snapshots became full backups, so instance DR rides the same rails. The
//! routes moved from `/api/admin/dr-backup/*` to `/api/admin/backup/*` once
//! nothing but the name was left of that subsystem.

use axum::extract::State;
use serde_json::json;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::response::ApiOk;

fn require_admin(auth: &AuthUser) -> Result<(), ApiError> {
    if crate::auth::guard::is_app_admin(auth.role) {
        Ok(())
    } else {
        Err(ApiError::Forbidden("admin only".into()))
    }
}

// ---------------- admin targets (spec §14) ----------------

use crate::target::repo as target_repo;

/// GET /api/admin/backup/targets — list (secrets masked).
pub async fn list_targets(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    let rows = target_repo::list_targets(&state.db).await?;
    let masked: Vec<serde_json::Value> = rows.iter().map(target_repo::mask_row).collect();
    Ok(ApiOk::ok(json!(masked)))
}

/// POST /api/admin/backup/targets — create.
pub async fn create_target(
    State(state): State<AppState>,
    auth: AuthUser,
    body: axum::Json<target_repo::TargetInput>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    let row = target_repo::upsert_target(&state.db, None, &body).await?;
    Ok(ApiOk::ok(target_repo::mask_row(&row)))
}

/// PUT /api/admin/backup/targets/:id — edit (gated on connection changes).
pub async fn update_target(
    State(state): State<AppState>,
    auth: AuthUser,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
    body: axum::Json<target_repo::TargetInput>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    let row = target_repo::upsert_target(&state.db, Some(id), &body).await?;
    Ok(ApiOk::ok(target_repo::mask_row(&row)))
}

/// POST /api/admin/backup/targets/:id/delete-check — probe before delete.
pub async fn delete_check_target(
    State(state): State<AppState>,
    auth: AuthUser,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    let row = target_repo::get_target(&state.db, id).await?;
    let probe = target_repo::probe_row(&row).await?;
    target_repo::delete_check(&state.db, id).await?;
    Ok(ApiOk::ok(json!({
        "deletable": true,
        "reachable": probe.reachable,
        "snapshot_objects": probe.snapshot_objects,
    })))
}

/// DELETE /api/admin/backup/targets/:id — gated delete.
pub async fn delete_target(
    State(state): State<AppState>,
    auth: AuthUser,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    target_repo::delete_check(&state.db, id).await?;
    sqlx::query("DELETE FROM backup_targets WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(ApiError::from)?;
    Ok(ApiOk::ok(json!("Target deleted.")))
}

/// POST /api/admin/backup/targets/:id/test — connectivity probe.
pub async fn test_target(
    State(state): State<AppState>,
    auth: AuthUser,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    let row = target_repo::get_target(&state.db, id).await?;
    let probe = target_repo::probe_row(&row).await?;
    Ok(ApiOk::ok(json!({
        "reachable": probe.reachable,
        "snapshot_objects": probe.snapshot_objects,
        "fingerprint": probe.fingerprint.map(|f| f.0),
        "owner_instance": probe.owner_instance,
        "owned_by_me": probe.owned_by_me,
    })))
}

/// POST /api/admin/backup/targets/probe-draft — probe connection fields that
/// have not been saved yet (siyuan #15 §9: the save button stays disabled until
/// a test passes). `***` credentials are resolved against the stored row when
/// `id` is supplied, so an untouched password still tests the real store.
///
/// Returns a `connection_token` the client echoes back on save; the server
/// recomputes it there to prove the tested connection is the saved one.
pub async fn probe_draft_target(
    State(state): State<AppState>,
    auth: AuthUser,
    axum::Json(body): axum::Json<ProbeDraftBody>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    let mut input = body.target;
    // Rehydrate masked secrets from the stored row (edit mode).
    if let Some(id) = body.id {
        let old = target_repo::get_target(&state.db, id).await?;
        input.access_key = target_repo::resolve_secret_pub(&input.access_key, &old.access_key);
        input.secret_key = target_repo::resolve_secret_pub(&input.secret_key, &old.secret_key);
    } else if input.access_key == target_repo::MASK || input.secret_key == target_repo::MASK {
        return Err(ApiError::BadRequest(
            "new target cannot test with *** credentials".into(),
        ));
    }
    let candidate =
        target_repo::row_from_input_pub(&input, crate::auth::service::now_ms(), body.id);
    let probe = target_repo::probe_row(&candidate).await?;
    Ok(ApiOk::ok(json!({
        "reachable": probe.reachable,
        "snapshot_objects": probe.snapshot_objects,
        "fingerprint": probe.fingerprint.map(|f| f.0),
        "owner_instance": probe.owner_instance,
        "owned_by_me": probe.owned_by_me,
        "connection_token": target_repo::connection_token(&input),
    })))
}

#[derive(Debug, serde::Deserialize)]
pub struct ProbeDraftBody {
    #[serde(default)]
    pub id: Option<uuid::Uuid>,
    pub target: target_repo::TargetInput,
}

/// POST /api/admin/backup/targets/:id/take-ownership — explicit takeover
/// (siyuan #15 §3: "last configured instance wins", but only on request).
pub async fn take_ownership(
    State(state): State<AppState>,
    auth: AuthUser,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    let row = target_repo::get_target(&state.db, id).await?;
    let conn = row.connect().await?;
    let previous = crate::target::owner::read_owner(&conn).await?;
    crate::target::owner::claim(&conn, true).await?;
    let now = crate::target::owner::current_owner();
    Ok(ApiOk::ok(json!({
        "instance_id": now.instance_id,
        "previous_instance": previous.map(|p| p.instance_id),
    })))
}

/// POST /api/admin/backup/targets/:id/empty — clear all objects.
pub async fn empty_target(
    State(state): State<AppState>,
    auth: AuthUser,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    if crate::locks::global().is_restoring().await {
        return Err(ApiError::Conflict(
            "a restore is running; target emptying must wait".into(),
        ));
    }
    let deleted = target_repo::empty_target(&state.db, id).await?;
    Ok(ApiOk::ok(json!({ "deleted_objects": deleted })))
}

#[derive(Debug, serde::Deserialize)]
pub struct SyncTargetBody {
    #[serde(default)]
    pub user_id: Option<String>,
}

/// POST /api/admin/backup/targets/:id/sync — run a snapshot now for the
/// target's users (direct-write: snapshots land on all of each user's
/// targets, not just this one). Without user_id, snapshots every user
/// assigned to the target. Returns the spawned job ids.
pub async fn sync_target(
    State(state): State<AppState>,
    auth: AuthUser,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
    body: Option<axum::Json<SyncTargetBody>>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    let storage = state
        .storage
        .clone()
        .ok_or(ApiError::NotFound("storage not configured".into()))?;
    let row = target_repo::get_target(&state.db, id).await?;
    let users: Vec<uuid::Uuid> = match body.and_then(|b| b.0.user_id) {
        Some(uid) => uid
            .parse::<uuid::Uuid>()
            .map(|v| vec![v])
            .map_err(|_| ApiError::BadRequest("user_id must be a UUID".into()))?,
        None => row.users.clone(),
    };
    let mut jobs = Vec::new();
    for uid in users {
        let options = crate::backup::BackupOptions {
            include_documents: true,
            include_files: true,
            include_settings: true,
            include_metadata: true,
            memo: Some(format!("Manual sync ({})", row.name)),
        };
        let job_id = state.backups.create(uid, options.clone()).await?;
        let store = state.backups.clone();
        let sql = state.db.clone();
        let storage2 = storage.clone();
        let spawn_job_id = job_id.clone();
        tokio::spawn(async move {
            crate::backup::run_backup(
                store,
                sql,
                storage2,
                uid,
                spawn_job_id,
                options,
                crate::snapcrypto::passphrase_from_env(),
                crate::backup::INLINE_ZIP_TTL_MS,
            )
            .await;
        });
        jobs.push(json!({ "user_id": uid.to_string(), "job_id": job_id }));
    }
    Ok(ApiOk::ok(json!(jobs)))
}

/// GET /api/admin/backup/users — users that own data (candidates
/// for target assignment). Admin only; ids are UUID strings on the wire.
pub async fn list_backup_users(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    let rows: Vec<(uuid::Uuid, String, Option<String>)> = sqlx::query_as(
        "SELECT DISTINCT u.id, u.username, u.email FROM users u \
         JOIN nodes n ON n.user_id = u.id ORDER BY u.username",
    )
    .fetch_all(&state.db)
    .await
    .map_err(ApiError::from)?;
    let out: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|(id, username, email)| {
            json!({
                "id": id.to_string(),
                "username": username,
                "email": email.unwrap_or_default(),
            })
        })
        .collect();
    Ok(ApiOk::ok(json!(out)))
}

/// GET /api/admin/backup/status — lock state for the frontend read-only
/// banner (also reachable while locked, see the middleware exemptions).
pub async fn lock_status(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<ApiOk<crate::locks::LockSnapshot>, ApiError> {
    require_admin(&auth)?;
    Ok(ApiOk::ok(state.locks.snapshot().await))
}

/// GET /api/admin/backup/targets/:id/export/:snapshot_id — download a
/// portable zip of one snapshot stored on a target (fork §6 "export zip").
pub async fn export_target_snapshot(
    State(state): State<AppState>,
    auth: AuthUser,
    axum::extract::Path((target_id, snapshot_id)): axum::extract::Path<(uuid::Uuid, String)>,
) -> Result<axum::response::Response, ApiError> {
    require_admin(&auth)?;
    if snapshot_id.is_empty() || !snapshot_id.chars().all(|c| c.is_ascii_digit()) {
        return Err(ApiError::BadRequest("invalid snapshot id".into()));
    }
    let row = target_repo::get_target(&state.db, target_id).await?;
    let conn = row.connect().await?;
    let manifest_key = format!("{snapshot_id}.json");
    let objects = conn.list(conn.list_prefix()).await?;
    let manifest_obj = objects
        .iter()
        .find(|(k, _, _)| k.ends_with(&format!("/manifests/{manifest_key}")))
        .map(|(k, _, _)| k.clone())
        .ok_or(ApiError::NotFound("snapshot not found on target".into()))?;
    let raw = conn.get(&manifest_obj).await?;
    let manifest: crate::backup::BackupManifest = serde_json::from_slice(&raw)
        .map_err(|e| ApiError::BadRequest(format!("manifest unreadable: {e}")))?;
    // Assemble the portable set straight from target objects (no local copy).
    use axum::response::IntoResponse;
    let mut archive = crate::backup::SimpleArchive::new();
    let meta = crate::restore::PortableMeta {
        format: crate::restore::PORTABLE_FORMAT.into(),
        version: crate::restore::PORTABLE_VERSION.into(),
        created_at: manifest.created_at.clone(),
        snapshot_id: manifest.id.clone(),
        user_id: manifest.user_id.clone(),
        encryption: manifest.encryption.clone(),
        entries: manifest.entries.clone(),
        files: manifest.files.clone(),
    };
    archive.add(
        "meta.json",
        &serde_json::to_vec_pretty(&meta).unwrap_or_default(),
    );
    archive.add("manifest.json", &raw);
    let mut seen = std::collections::HashSet::new();
    let mut hashes: Vec<String> = manifest.entries.iter().map(|e| e.sha256.clone()).collect();
    hashes.extend(manifest.files.iter().map(|f| f.sha256.clone()));
    for sha in hashes {
        if !seen.insert(sha.clone()) {
            continue;
        }
        let dir = if manifest.encryption.is_some() {
            "blobs-e2e"
        } else {
            "blobs"
        };
        let key = objects
            .iter()
            .find(|(k, _, _)| k.ends_with(&format!("/{dir}/{sha}")))
            .map(|(k, _, _)| k.clone())
            .ok_or(ApiError::NotFound(format!("blob {sha} missing on target")))?;
        archive.add_stored(format!("blobs/{sha}"), &conn.get(&key).await?);
    }
    let bytes = archive
        .finish()
        .map_err(|e| ApiError::BadRequest(format!("export assembly failed: {e}")))?;
    Ok((
        axum::http::StatusCode::OK,
        [
            (axum::http::header::CONTENT_TYPE, "application/zip"),
            (
                axum::http::header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"pergame-snapshot-{snapshot_id}.zip\"").as_str(),
            ),
        ],
        bytes,
    )
        .into_response())
}
