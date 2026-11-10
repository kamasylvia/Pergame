//! Fan-out engine (spec §14.5): after a snapshot lands in the local bucket,
//! replicate that user's objects (manifests + blobs + refs) to every enabled
//! target listing the user. Objects are copied byte-for-byte (encrypted blobs
//! stay ciphertext — no decrypt/re-encrypt), and existing objects are skipped
//! so only new content is transferred (incremental).

use serde::Serialize;
use sqlx::PgPool;

use super::repo;
use crate::backup::SNAPSHOT_KEEP;
use crate::storage::S3Storage;

#[derive(Debug, Default, Serialize)]
pub struct FanoutResult {
    pub target: String,
    pub copied: usize,
    pub skipped: usize,
    pub pruned: usize,
    pub error: Option<String>,
}

/// Collects one user's snapshot objects from the local backup bucket.
async fn collect_user_objects(
    storage: &S3Storage,
    user_id: i64,
) -> Result<Vec<(String, Vec<u8>)>, crate::error::ApiError> {
    let mut keys: Vec<String> = Vec::new();
    for (k, _, _) in storage.list_manifests(user_id).await? {
        keys.push(k);
    }
    for (k, _, _) in storage.list_backup_snapshots(user_id).await? {
        keys.push(k); // legacy v1 zips + sidecars stay restorable offsite too
    }
    for k in storage.list_blob_keys(user_id).await? {
        keys.push(k);
    }
    keys.sort();
    keys.dedup();

    let mut out = Vec::with_capacity(keys.len());
    for key in keys {
        match storage.get_backup_object(&key).await {
            Ok(bytes) => out.push((key, bytes)),
            // Must be visible: a silently skipped manifest means the offsite
            // replica is incomplete even though fan-out reports success.
            Err(e) => tracing::warn!(%key, error = %e, "fan-out: local object unreadable"),
        }
    }
    tracing::info!(
        user_id,
        objects = out.len(),
        "fan-out: collected local objects"
    );
    Ok(out)
}

/// Replicates one user's objects to every enabled target that lists the user.
/// Called right after a successful snapshot (auto) and by the admin "sync now"
/// action. Best-effort per target: a failure is recorded, others continue.
pub async fn fanout_user(pool: &PgPool, storage: &S3Storage, user_id: i64) -> Vec<FanoutResult> {
    let mut results = Vec::new();
    let targets = match repo::list_targets(pool).await {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(error = %e, "fan-out: target list failed");
            return results;
        }
    };
    let entries = match collect_user_objects(storage, user_id).await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, user_id, "fan-out: object collection failed");
            return results;
        }
    };
    if entries.is_empty() {
        return results;
    }
    for target in targets
        .into_iter()
        .filter(|t| t.enabled && t.users.contains(&user_id))
    {
        let mut result = FanoutResult {
            target: target.name.clone(),
            ..Default::default()
        };
        match target.connect().await {
            Ok(conn) => {
                match conn.replicate(entries.clone()).await {
                    Ok((copied, skipped)) => {
                        result.copied = copied;
                        result.skipped = skipped;
                        // Retention on the target mirrors the local keep-last.
                        if let Err(e) = prune_target(&conn, &target.prefix, target.keep_last).await
                        {
                            tracing::warn!(error = %e, target = %target.name, "fan-out prune failed");
                        }
                    }
                    Err(e) => result.error = Some(e.to_string()),
                }
            }
            Err(e) => result.error = Some(e.to_string()),
        }
        let summary = match &result.error {
            Some(e) => format!("error: {e}"),
            None => format!("copied {}, skipped {}", result.copied, result.skipped),
        };
        let _ = sqlx::query(
            "UPDATE backup_targets SET last_result = $2, updated_at = $3 WHERE id = $1",
        )
        .bind(target.id)
        .bind(&summary)
        .bind(crate::auth::service::now_ms())
        .execute(pool)
        .await;
        results.push(result);
    }
    results
}

/// Prunes the oldest manifests on a target beyond `keep_last`, then deletes
/// blobs no longer referenced by any surviving manifest.
async fn prune_target(
    conn: &super::config::TargetConn,
    prefix: &str,
    keep_last: i32,
) -> Result<usize, crate::error::ApiError> {
    let keep = keep_last.max(1) as usize;
    let objects = conn.list(conn.list_prefix(prefix)).await?;
    let mut manifests: Vec<String> = objects
        .iter()
        .filter(|(k, _, _)| k.contains("/manifests/"))
        .map(|(k, _, _)| k.clone())
        .collect();
    if manifests.len() <= keep {
        return Ok(0);
    }
    manifests.sort();
    let excess = manifests.len() - keep;
    let mut pruned = 0usize;
    for key in manifests.into_iter().take(excess) {
        if conn.delete(&key).await.is_ok() {
            pruned += 1;
        }
    }
    // Blob GC: keep only hashes referenced by surviving manifests.
    let mut referenced = std::collections::HashSet::new();
    for (k, _, _) in conn.list(conn.list_prefix(prefix)).await? {
        if !k.contains("/manifests/") {
            continue;
        }
        if let Ok(bytes) = conn.get(&k).await
            && let Ok(m) = serde_json::from_slice::<crate::backup::BackupManifest>(&bytes)
        {
            for e in m.entries {
                referenced.insert(e.sha256);
            }
            for f in m.files {
                referenced.insert(f.sha256);
            }
        }
    }
    for (k, _, _) in conn.list(conn.list_prefix(prefix)).await? {
        if !k.contains("/blobs") {
            continue;
        }
        let sha = k.rsplit('/').next().unwrap_or("");
        if !referenced.contains(sha) && conn.delete(&k).await.is_ok() {
            pruned += 1;
        }
    }
    let _ = SNAPSHOT_KEEP; // local retention constant kept as documentation anchor
    Ok(pruned)
}

/// Pulls one user's snapshot objects from a target back into the local bucket
/// (only missing ones), so the existing restore engine can consume them.
pub async fn fetch_user_from_target(
    pool: &PgPool,
    storage: &S3Storage,
    target_id: uuid::Uuid,
    user_id: i64,
) -> Result<usize, crate::error::ApiError> {
    let target = repo::get_target(pool, target_id).await?;
    let conn = target.connect().await?;
    let prefix = format!("{}/", user_id);
    let objects = conn.list(conn.list_prefix(&target.prefix)).await?;
    let mut fetched = 0usize;
    for (key, _, _) in objects {
        if !key.starts_with(&prefix) {
            continue;
        }
        let local_exists = storage.get_backup_object(&key).await.is_ok();
        if local_exists {
            continue;
        }
        if let Ok(bytes) = conn.get(&key).await
            && storage.put_backup_object(&key, bytes).await.is_ok()
        {
            fetched += 1;
        }
    }
    Ok(fetched)
}
