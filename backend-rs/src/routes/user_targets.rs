//! Per-user backup targets (`/api/backup/targets/*`).
//!
//! Same store as the admin targets API (`/api/admin/backup/targets/*`),
//! but scoped: a regular user only sees and manages targets that list them,
//! and can only ever assign themselves. Admins keep the old routes for
//! instance-wide management (multi-user assignment, takeover).
//!
//! Design note: this is deliberately a second set of handlers over the same
//! `target::repo` functions rather than a relaxed admin route — the URL
//! namespace says who may call it, and the `users` scoping rule lives in one
//! place per route family instead of branching inside shared code.

use axum::Json;
use axum::extract::{Path, State};
use serde_json::json;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::response::ApiOk;
use crate::target::repo as target_repo;

/// Admins pass; regular users must be listed on the target row.
async fn require_owner(
    db: &sqlx::PgPool,
    auth: &AuthUser,
    id: Uuid,
) -> Result<crate::target::config::BackupTargetRow, ApiError> {
    let row = target_repo::get_target(db, id).await?;
    if crate::auth::guard::is_app_admin(auth.role) || row.users.contains(&auth.user_id()) {
        Ok(row)
    } else {
        Err(ApiError::Forbidden("not your backup target".into()))
    }
}

/// Regular users may only ever assign themselves; admins keep free choice.
fn scope_users(auth: &AuthUser, input: &mut target_repo::TargetInput) {
    if !crate::auth::guard::is_app_admin(auth.role) {
        input.users = vec![auth.user_id()];
    }
}

/// GET /api/backup/targets — caller's targets (secrets masked).
pub async fn list_targets(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let rows = target_repo::list_targets(&state.db).await?;
    let mine: Vec<serde_json::Value> = rows
        .iter()
        .filter(|r| {
            crate::auth::guard::is_app_admin(auth.role) || r.users.contains(&auth.user_id())
        })
        .map(target_repo::mask_row)
        .collect();
    Ok(ApiOk::ok(json!(mine)))
}

/// POST /api/backup/targets — create (users forced to [self]).
pub async fn create_target(
    State(state): State<AppState>,
    auth: AuthUser,
    body: Json<target_repo::TargetInput>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let mut input = body.0;
    scope_users(&auth, &mut input);
    let row = target_repo::upsert_target(&state.db, None, &input).await?;
    Ok(ApiOk::ok(target_repo::mask_row(&row)))
}

/// PUT /api/backup/targets/:id — edit (gated on connection changes).
pub async fn update_target(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    body: Json<target_repo::TargetInput>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_owner(&state.db, &auth, id).await?;
    let mut input = body.0;
    scope_users(&auth, &mut input);
    let row = target_repo::upsert_target(&state.db, Some(id), &input).await?;
    Ok(ApiOk::ok(target_repo::mask_row(&row)))
}

/// POST /api/backup/targets/:id/delete-check — probe before delete.
pub async fn delete_check_target(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let row = require_owner(&state.db, &auth, id).await?;
    let probe = target_repo::probe_row(&row).await?;
    target_repo::delete_check(&state.db, id).await?;
    Ok(ApiOk::ok(json!({
        "deletable": true,
        "reachable": probe.reachable,
        "snapshot_objects": probe.snapshot_objects,
    })))
}

/// DELETE /api/backup/targets/:id — gated delete.
pub async fn delete_target(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_owner(&state.db, &auth, id).await?;
    target_repo::delete_check(&state.db, id).await?;
    sqlx::query("DELETE FROM backup_targets WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(ApiError::from)?;
    Ok(ApiOk::ok(json!("Target deleted.")))
}

/// POST /api/backup/targets/:id/test — connectivity probe.
pub async fn test_target(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let row = require_owner(&state.db, &auth, id).await?;
    let probe = target_repo::probe_row(&row).await?;
    Ok(ApiOk::ok(json!({
        "reachable": probe.reachable,
        "snapshot_objects": probe.snapshot_objects,
        "fingerprint": probe.fingerprint.map(|f| f.0),
        "owner_instance": probe.owner_instance,
        "owned_by_me": probe.owned_by_me,
    })))
}

/// POST /api/backup/targets/probe-draft — probe unsaved connection fields
/// (siyuan #15 §9 save gate). Same semantics as the admin endpoint; edit
/// mode additionally requires ownership of the referenced row.
pub async fn probe_draft_target(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<crate::routes::admin_backup::ProbeDraftBody>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let mut input = body.target;
    if let Some(id) = body.id {
        let old = require_owner(&state.db, &auth, id).await?;
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

/// POST /api/backup/targets/:id/empty — clear all objects.
pub async fn empty_target(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_owner(&state.db, &auth, id).await?;
    if crate::locks::global().is_restoring().await {
        return Err(ApiError::Conflict(
            "a restore is running; target emptying must wait".into(),
        ));
    }
    let deleted = target_repo::empty_target(&state.db, id).await?;
    Ok(ApiOk::ok(json!({ "deleted_objects": deleted })))
}

/// POST /api/backup/targets/:id/sync — run a snapshot of the caller's data
/// now (direct-write: lands on all of the caller's targets). Returns the
/// spawned job id.
pub async fn sync_target(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let row = require_owner(&state.db, &auth, id).await?;
    let storage = state
        .storage
        .clone()
        .ok_or(ApiError::NotFound("storage not configured".into()))?;
    let options = crate::backup::BackupOptions {
        include_documents: true,
        include_files: true,
        include_settings: true,
        include_metadata: true,
        memo: Some(format!("Manual sync ({})", row.name)),
    };
    let job_id = state
        .backups
        .create(auth.user_id(), options.clone())
        .await?;
    let store = state.backups.clone();
    let sql = state.db.clone();
    let uid = auth.user_id();
    let spawn_job_id = job_id.clone();
    tokio::spawn(async move {
        crate::backup::run_backup(
            store,
            sql,
            storage,
            uid,
            spawn_job_id,
            options,
            None,
            crate::backup::INLINE_ZIP_TTL_MS,
        )
        .await;
    });
    Ok(ApiOk::ok(json!({ "job_id": job_id })))
}

/// GET /api/backup/targets/:id/export/:snapshotId — portable zip of one
/// snapshot, assembled straight from target objects (no local copy).
pub async fn export_target_snapshot(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((target_id, snapshot_id)): Path<(Uuid, String)>,
) -> Result<axum::response::Response, ApiError> {
    use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
    let row = require_owner(&state.db, &auth, target_id).await?;
    if snapshot_id.is_empty() || !snapshot_id.chars().all(|c| c.is_ascii_digit()) {
        return Err(ApiError::BadRequest("invalid snapshot id".into()));
    }
    let conn = row.connect().await?;
    let manifest_key = format!("{snapshot_id}.json");
    let objects = conn.list(conn.list_prefix()).await?;
    let want = format!("{}/manifests/{manifest_key}", auth.user_id());
    let manifest_obj = objects
        .iter()
        .find(|(k, _, _)| k.ends_with(&want))
        .map(|(k, _, _)| k.clone())
        .ok_or(ApiError::NotFound("snapshot not found on target".into()))?;
    let raw = conn.get(&manifest_obj).await?;
    let manifest: crate::backup::BackupManifest = serde_json::from_slice(&raw)
        .map_err(|e| ApiError::BadRequest(format!("manifest unreadable: {e}")))?;
    if manifest.user_id != auth.user_id().to_string()
        && !crate::auth::guard::is_app_admin(auth.role)
    {
        return Err(ApiError::Forbidden("not your snapshot".into()));
    }
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
            (CONTENT_TYPE, "application/zip"),
            (
                CONTENT_DISPOSITION,
                format!("attachment; filename=\"pergame-snapshot-{snapshot_id}.zip\"").as_str(),
            ),
        ],
        bytes,
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regular_users_scoped_to_self() {
        let me = Uuid::now_v7();
        let auth = AuthUser { id: me, role: 1 };
        let mut input = target_repo::TargetInput {
            kind: "s3".into(),
            name: "t".into(),
            endpoint: String::new(),
            bucket: String::new(),
            prefix: String::new(),
            access_key: String::new(),
            secret_key: String::new(),
            secure: true,
            region: "us-east-1".into(),
            provider: "Other".into(),
            keep_last: 10,
            max_age_days: 0,
            schedule: String::new(),
            enabled: true,
            users: vec![Uuid::nil(), Uuid::max()],
            namespace: String::new(),
            connection_token: None,
        };
        scope_users(&auth, &mut input);
        assert_eq!(input.users, vec![me]);
        // Admins keep free choice.
        let admin = AuthUser {
            id: Uuid::nil(),
            role: 2,
        };
        let mut input2 = input.clone();
        input2.users = vec![Uuid::nil(), Uuid::max()];
        scope_users(&admin, &mut input2);
        assert_eq!(input2.users.len(), 2);
    }
}
