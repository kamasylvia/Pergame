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
                // Ownership gate: only the claiming instance writes. A foreign
                // claim means another deployment owns this scope's lineage.
                match super::owner::claim(&conn, false).await {
                    Ok(super::owner::ClaimOutcome::Claimed) => {}
                    Ok(super::owner::ClaimOutcome::Foreign { owner, .. }) => {
                        result.error = Some(format!(
                            "target owned by instance '{}' (claimed {}); take over explicitly to write",
                            owner.instance_id, owner.claimed_at
                        ));
                        record_result(pool, &target, &result).await;
                        results.push(result);
                        continue;
                    }
                    Err(e) => {
                        result.error = Some(format!("ownership check failed: {e}"));
                        record_result(pool, &target, &result).await;
                        results.push(result);
                        continue;
                    }
                }
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
        record_result(pool, &target, &result).await;
        results.push(result);
    }
    results
}

/// Records the last fan-out outcome on the target row (operator visibility).
async fn record_result(
    pool: &PgPool,
    target: &super::config::BackupTargetRow,
    result: &FanoutResult,
) {
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

/// Per-target cron scheduler (fork §4 parity): ticks every minute, asks each
/// enabled target whether its `schedule` matched the elapsed minute, and runs
/// a full fan-out for that target's users. Missed minutes are not replayed
/// (standard cron), and a tick is skipped while a backup/restore lock is held.
pub fn spawn_target_scheduler(pool: sqlx::PgPool, storage: S3Storage) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(60));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        ticker.tick().await; // skip the immediate tick
        let mut last_minute: i64 = 0;
        loop {
            ticker.tick().await;
            let now = crate::auth::service::now_ms() / 1000;
            let minute = now - now % 60;
            if minute == last_minute {
                continue;
            }
            last_minute = minute;
            if crate::locks::global().is_read_only().await {
                tracing::info!("target scheduler: skipped, backup/restore in flight");
                continue;
            }
            let rows: Vec<(uuid::Uuid, String, String)> = match sqlx::query_as(
                "SELECT id, name, schedule FROM backup_targets \
                 WHERE enabled = true AND schedule <> ''",
            )
            .fetch_all(&pool)
            .await
            {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(error = %e, "target scheduler: schedule query failed");
                    continue;
                }
            };
            for (id, name, expr) in rows {
                let Ok(spec) = crate::target::cron::CronSpec::parse(&expr) else {
                    tracing::warn!(target = %name, expr, "target scheduler: bad cron expression");
                    continue;
                };
                if !spec.matches_utc(minute) {
                    continue;
                }
                let Ok(row) = repo::get_target(&pool, id).await else {
                    continue;
                };
                tracing::info!(target = %name, "target scheduler: running scheduled fan-out");
                for uid in row.users.clone() {
                    for r in fanout_user(&pool, &storage, uid).await {
                        if let Some(e) = r.error {
                            tracing::warn!(target = %r.target, error = %e, "scheduled fan-out failed");
                        }
                    }
                }
            }
        }
    });
}
