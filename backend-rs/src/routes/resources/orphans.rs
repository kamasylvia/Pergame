//! /resources orphan routes — scan, list, and purge unreferenced stored objects.

use axum::Json;
use axum::extract::State;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::repo::nodes as node_repo;
use crate::response::ApiOk;

/// Objects under `{uid}/` that no resource node references, plus blob-store
/// rows no node points at (crashed uploads, direct row deletes). Files of
/// trashed nodes count as referenced (they must survive until the trash is
/// purged), and the avatar plus the `inline-zips/` prefix are never
/// candidates.
async fn scan_orphans(
    state: &AppState,
    user_id: Uuid,
) -> Result<(Vec<serde_json::Value>, i64), ApiError> {
    let storage = state
        .storage
        .clone()
        .ok_or(ApiError::BadRequest("minio client not initialized".into()))?;
    let referenced: std::collections::HashSet<String> =
        node_repo::referenced_resource_paths(&state.db, user_id)
            .await
            .map_err(ApiError::from)?
            .into_iter()
            .collect();

    let objects = storage.list_user_objects(user_id).await?;
    let mut out = Vec::new();
    let mut total = 0i64;
    for (name, size, mtime) in objects {
        if name == "avatar" || referenced.contains(&name) {
            continue;
        }
        total += size;
        out.push(json!({
            "key": name,
            "size": size,
            "last_modified": mtime,
        }));
    }
    // Dereferenced blobs (no node carries the sha): report as blobs/{sha}
    // so purge can remove the object and the row together.
    let stray: Vec<(String, i64)> = sqlx::query_as(
        "SELECT sha256, size FROM object_blobs b
         WHERE NOT EXISTS (
             SELECT 1 FROM nodes n
             WHERE n.metadata->>'content_sha256' = b.sha256
         )",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();
    for (sha, size) in stray {
        total += size;
        out.push(json!({
            "key": format!("{}/{}", crate::blobs::BLOBS_PREFIX, sha),
            "size": size,
        }));
    }
    out.sort_by(|a, b| b["size"].as_i64().cmp(&a["size"].as_i64()));
    Ok((out, total))
}

/// GET /resources/orphans — list stored objects nothing points at.
pub async fn list_orphans(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let (objects, total) = scan_orphans(&state, auth.user_id()).await?;
    Ok(ApiOk::ok(json!({
        "objects": objects,
        "total_bytes": total,
        "count": objects.len(),
    })))
}

#[derive(Debug, Deserialize)]
/// POST purge body (keys to delete; null = all scanned orphans).
pub struct PurgeOrphansInput {
    /// Object names to delete; omitted/null = every orphan found by the scan.
    #[serde(default)]
    pub keys: Option<Vec<String>>,
}

/// POST /resources/orphans/purge — delete orphans (all, or the named ones).
/// The scan is repeated here: a file that a node started referencing between
/// listing and purging is not deleted.
pub async fn purge_orphans(
    State(state): State<AppState>,
    auth: AuthUser,
    body: Option<Json<PurgeOrphansInput>>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let storage = state
        .storage
        .clone()
        .ok_or(ApiError::BadRequest("minio client not initialized".into()))?;
    let (objects, _) = scan_orphans(&state, auth.user_id()).await?;
    let orphans: std::collections::HashSet<String> = objects
        .iter()
        .filter_map(|o| o["key"].as_str().map(str::to_string))
        .collect();

    let requested = body.and_then(|b| b.0.keys);
    let targets: Vec<String> = match requested {
        Some(keys) => keys.into_iter().filter(|k| orphans.contains(k)).collect(),
        None => orphans.into_iter().collect(),
    };

    let mut deleted = 0usize;
    let mut bytes = 0i64;
    for key in &targets {
        // Blob keys remove the row + object together; legacy keys delete
        // the per-user object.
        let removed = match key.strip_prefix(&format!("{}/", crate::blobs::BLOBS_PREFIX)) {
            Some(sha) => {
                let _ = sqlx::query("DELETE FROM object_blobs WHERE sha256 = $1")
                    .bind(sha)
                    .execute(&state.db)
                    .await;
                storage.delete_blob(sha).await;
                true
            }
            None => storage
                .delete_user_object_checked(auth.user_id(), key)
                .await
                .is_ok(),
        };
        if removed {
            deleted += 1;
            bytes += objects
                .iter()
                .find(|o| o["key"].as_str() == Some(key))
                .and_then(|o| o["size"].as_i64())
                .unwrap_or(0);
        }
    }
    tracing::info!(%deleted, %bytes, "orphan resources purged");
    Ok(ApiOk::ok(json!({ "deleted": deleted, "bytes": bytes })))
}

#[derive(Debug, serde::Deserialize)]
/// GET preview query (orphan key from the scan list).
pub struct PreviewOrphanQuery {
    #[serde(default)]
    pub key: String,
}

/// GET /resources/orphans/preview?key= — stream one orphan's bytes inline
/// for pre-delete inspection. The key must still be in the current scan
/// (same rescan guard as purge); MIME is guessed from the extension.
pub async fn preview_orphan(
    State(state): State<AppState>,
    auth: AuthUser,
    axum::extract::Query(q): axum::extract::Query<PreviewOrphanQuery>,
    headers: axum::http::HeaderMap,
) -> Result<axum::response::Response, ApiError> {
    use axum::http::{StatusCode, header};
    use axum::response::IntoResponse;
    let storage = state
        .storage
        .clone()
        .ok_or(ApiError::BadRequest("minio client not initialized".into()))?;
    let (objects, _) = scan_orphans(&state, auth.user_id()).await?;
    let found = objects
        .iter()
        .any(|o| o["key"].as_str() == Some(q.key.as_str()));
    if !found || q.key.is_empty() {
        return Err(ApiError::NotFound("not an orphan".into()));
    }
    let blob_prefix = format!("{}/", crate::blobs::BLOBS_PREFIX);
    let bytes = match q.key.strip_prefix(&blob_prefix) {
        Some(sha) => storage.get_blob(sha).await?,
        None => storage.get_user_object(auth.user_id(), &q.key).await?,
    };
    let mime = match q.key.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()) {
        Some(e)
            if ["png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "avif"]
                .contains(&e.as_str()) =>
        {
            format!("image/{}", if e == "jpg" { "jpeg" } else { e.as_str() })
        }
        Some(e) if ["mp4", "webm", "ogv", "mov", "mkv"].contains(&e.as_str()) => {
            format!("video/{e}")
        }
        Some(e) if ["mp3", "wav", "ogg", "flac", "m4a"].contains(&e.as_str()) => {
            format!("audio/{e}")
        }
        Some(e) if e == "pdf" => "application/pdf".into(),
        Some(e) if ["txt", "md", "json", "csv", "html", "css", "js"].contains(&e.as_str()) => {
            format!("text/{}", if e == "txt" { "plain" } else { e.as_str() })
        }
        _ => "application/octet-stream".into(),
    };
    let _ = headers;
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, mime),
            (header::ACCEPT_RANGES, "bytes".into()),
        ],
        bytes,
    )
        .into_response())
}
