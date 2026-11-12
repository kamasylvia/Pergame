//! Direct-write snapshot engine: a snapshot lands on the user's own
//! targets, never on the service bucket. Zero targets = the snapshot is
//! packaged as a portable zip in memory for one download (fork §4 "save zip
//! locally"); nothing is persisted server-side.
//!
//! Every enabled target listing the user receives identical bytes
//! (multi-target full write with per-target dedup, fork §2). Failures are
//! per-target: one bad target never blocks the others, but all-failed fails
//! the job. Retention runs per target after a successful write.

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::repo;
use crate::backup::{BackupManifest, SNAPSHOT_KEEP};

#[derive(Debug, Default, Serialize)]
pub struct FanoutResult {
    pub target: String,
    pub copied: usize,
    pub skipped: usize,
    pub pruned: usize,
    pub error: Option<String>,
}

/// Writes one snapshot (manifest + member blobs + file blobs) to every
/// enabled target listing the user. Payloads are `(sha, e2e, bytes)`.
#[allow(clippy::too_many_arguments)]
pub async fn write_snapshot_to_targets(
    pool: &PgPool,
    user_id: Uuid,
    manifest: &BackupManifest,
    manifest_bytes: &[u8],
    member_payloads: &[(String, bool, Vec<u8>)],
    file_payloads: &[(String, bool, Vec<u8>)],
) -> Vec<FanoutResult> {
    let mut results = Vec::new();
    let targets = match repo::list_targets_for_user(pool, user_id).await {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(error = %e, "direct-write: target list failed");
            return results;
        }
    };
    let manifest_key = format!("{user_id}/manifests/{}.json", manifest.id);
    for target in targets {
        let mut result = FanoutResult {
            target: target.name.clone(),
            ..Default::default()
        };
        let conn = match target.connect().await {
            Ok(c) => c,
            Err(e) => {
                result.error = Some(e.to_string());
                record_result(pool, &target, &result).await;
                results.push(result);
                continue;
            }
        };
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
        // Blob entries: members + files, content-addressed, deduped per target.
        let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
        for (sha, e2e, bytes) in member_payloads.iter().chain(file_payloads.iter()) {
            let dir = if *e2e { "blobs-e2e" } else { "blobs" };
            entries.push((format!("{user_id}/{dir}/{sha}"), bytes.clone()));
        }
        match conn.replicate(entries).await {
            Ok((copied, skipped)) => {
                result.copied = copied;
                result.skipped = skipped;
            }
            Err(e) => {
                result.error = Some(e.to_string());
                record_result(pool, &target, &result).await;
                results.push(result);
                continue;
            }
        }
        if let Err(e) = conn.put(&manifest_key, manifest_bytes.to_vec()).await {
            result.error = Some(format!("manifest upload failed: {e}"));
            record_result(pool, &target, &result).await;
            results.push(result);
            continue;
        }
        result.copied += 1;
        // Retention on the target mirrors the local keep-last.
        match prune_target(&conn, target.keep_last).await {
            Ok(pruned) => result.pruned = pruned,
            Err(e) => {
                tracing::warn!(error = %e, target = %target.name, "direct-write prune failed")
            }
        }
        record_result(pool, &target, &result).await;
        results.push(result);
    }
    results
}

/// Records the last write outcome on the target row (operator visibility).
async fn record_result(
    pool: &PgPool,
    target: &super::config::BackupTargetRow,
    result: &FanoutResult,
) {
    let summary = if let Some(e) = &result.error {
        format!("error: {e}")
    } else {
        format!(
            "ok: copied {} skipped {} pruned {}",
            result.copied, result.skipped, result.pruned
        )
    };
    let _ =
        sqlx::query("UPDATE backup_targets SET last_result = $2, updated_at = $3 WHERE id = $1")
            .bind(target.id)
            .bind(&summary)
            .bind(crate::auth::service::now_ms())
            .execute(pool)
            .await;
}

/// Prunes the oldest manifests on a target beyond `keep_last`, then deletes
/// blobs no longer referenced by any surviving manifest.
pub async fn prune_target(
    conn: &super::config::TargetConn,
    keep_last: i32,
) -> Result<usize, crate::error::ApiError> {
    let keep = keep_last.max(1) as usize;
    let objects = conn.list(conn.list_prefix()).await?;
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
    for (k, _, _) in conn.list(conn.list_prefix()).await? {
        if !k.contains("/manifests/") {
            continue;
        }
        if let Ok(bytes) = conn.get(&k).await
            && let Ok(m) = serde_json::from_slice::<BackupManifest>(&bytes)
        {
            for e in m.entries {
                referenced.insert(e.sha256);
            }
            for f in m.files {
                referenced.insert(f.sha256);
            }
        }
    }
    for (k, _, _) in conn.list(conn.list_prefix()).await? {
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

/// First reachable target connection for the user, for read paths
/// (list/download/restore/preview). All targets hold identical snapshots,
/// so reading one is enough. None = no targets or all unreachable, and the
/// caller reports "no snapshots visible" (never a partial view).
pub async fn first_readable_target(
    pool: &PgPool,
    user_id: Uuid,
) -> Option<super::config::TargetConn> {
    let targets = repo::list_targets_for_user(pool, user_id).await.ok()?;
    for target in targets {
        let Ok(conn) = target.connect().await else {
            continue;
        };
        // Reachability check: a list that errors means unusable.
        if conn.list(conn.list_prefix()).await.is_ok() {
            return Some(conn);
        }
    }
    None
}

/// All reachable target connections for the user (for delete: a snapshot
/// delete removes the manifest from every target holding it).
pub async fn readable_targets(
    pool: &PgPool,
    user_id: Uuid,
) -> Vec<(String, super::config::TargetConn)> {
    let mut out = Vec::new();
    let Ok(targets) = repo::list_targets_for_user(pool, user_id).await else {
        return out;
    };
    for target in targets {
        let Ok(conn) = target.connect().await else {
            continue;
        };
        if conn.list(conn.list_prefix()).await.is_ok() {
            out.push((target.name.clone(), conn));
        }
    }
    out
}

/// Per-target cron scheduler (fork §4 parity): ticks every minute, asks each
/// enabled target whether its `schedule` matched the elapsed minute, and runs
/// a direct-write snapshot for that target's users. Expressions are evaluated
/// in the process timezone (`TZ` env var; UTC in containers by default).
/// Missed minutes are not replayed (standard cron), and a tick is skipped
/// while a backup/restore lock is held.
pub fn spawn_target_scheduler(
    pool: sqlx::PgPool,
    store: std::sync::Arc<crate::backup::BackupStore>,
    storage: crate::storage::S3Storage,
) {
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
                if !spec.matches(minute) {
                    continue;
                }
                let Ok(row) = repo::get_target(&pool, id).await else {
                    continue;
                };
                tracing::info!(target = %name, "target scheduler: running scheduled snapshot");
                for uid in row.users.clone() {
                    let options = crate::backup::BackupOptions {
                        include_documents: true,
                        include_files: true,
                        include_settings: true,
                        include_metadata: true,
                        memo: Some(format!("Scheduled snapshot ({name})")),
                    };
                    let Ok(job_id) = store.create(uid, options.clone()).await else {
                        continue;
                    };
                    let store2 = store.clone();
                    let pool2 = pool.clone();
                    let storage2 = storage.clone();
                    tokio::spawn(async move {
                        crate::backup::run_backup(
                            store2,
                            pool2,
                            storage2,
                            uid,
                            job_id,
                            options,
                            crate::snapcrypto::passphrase_from_env(),
                        )
                        .await;
                    });
                }
            }
        }
    });
}
