//! Scheduled snapshots, inline-zip sweeping, and the in-memory zip assembler.

use std::sync::Arc;

use uuid::Uuid;

use super::run::run_backup;
use super::{BackupOptions, BackupStore, INLINE_ZIP_TTL_MS};

/// Minimal in-memory zip assembler (stored/deflated entries).
pub(crate) struct SimpleArchive {
    writer: Option<zip::ZipWriter<std::io::Cursor<Vec<u8>>>>,
}

impl SimpleArchive {
    pub(crate) fn new() -> Self {
        Self {
            writer: Some(zip::ZipWriter::new(std::io::Cursor::new(Vec::new()))),
        }
    }

    pub(crate) fn add(&mut self, name: &str, bytes: &[u8]) {
        self.add_with(name, bytes, zip::CompressionMethod::Deflated);
    }

    pub(crate) fn add_stored(&mut self, name: String, bytes: &[u8]) {
        self.add_with(name, bytes, zip::CompressionMethod::Stored);
    }

    fn add_with(&mut self, name: impl Into<String>, bytes: &[u8], method: zip::CompressionMethod) {
        let Some(writer) = self.writer.as_mut() else {
            return;
        };
        if writer
            .start_file(
                name.into(),
                zip::write::SimpleFileOptions::default().compression_method(method),
            )
            .is_ok()
        {
            std::io::Write::write_all(writer, bytes).ok();
        }
    }

    pub(crate) fn finish(mut self) -> Result<Vec<u8>, zip::result::ZipError> {
        let writer = self.writer.take().expect("archive writer");
        let cursor = writer.finish()?;
        Ok(cursor.into_inner())
    }
}

/// SHA-256 hex of raw bytes (blob identity for v2 snapshots).
pub(crate) fn hex_sha(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}

/// Scheduled snapshot ticker (spec backup-restore.md: automatic user-level
/// snapshots; each snapshot is a full backup direct-written to targets). Runs every
/// SNAPSHOT_INTERVAL_HOURS (env, unset/empty = disabled; must be > 0) and
/// snapshots each user that has data, memoed "Scheduled snapshot".
/// Retention/GC from the normal backup path applies, and a user's in-flight
/// manual job is never clobbered.
pub fn spawn_snapshot_scheduler(
    store: Arc<BackupStore>,
    sql: sqlx::PgPool,
    storage: crate::storage::S3Storage,
) {
    let raw = std::env::var("SNAPSHOT_INTERVAL_HOURS")
        .ok()
        .map(|v| v.trim().to_string());
    let hours: u64 = match raw.as_deref() {
        None | Some("") => {
            tracing::info!("SNAPSHOT_INTERVAL_HOURS unset/empty: scheduled snapshots disabled");
            return;
        }
        Some(v) => match v.parse() {
            Ok(h) if h > 0 => h,
            _ => {
                tracing::warn!(value = %v, "SNAPSHOT_INTERVAL_HOURS must be > 0; scheduled snapshots disabled");
                return;
            }
        },
    };
    tracing::info!(hours, "scheduled user snapshots enabled");
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(hours * 3600));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        // First tick fires immediately; skip it so boot is not a backup storm.
        ticker.tick().await;
        loop {
            ticker.tick().await;
            let users: Vec<(Uuid,)> = sqlx::query_as("SELECT DISTINCT user_id FROM nodes")
                .fetch_all(&sql)
                .await
                .unwrap_or_default();
            tracing::info!(users = users.len(), "scheduled snapshots starting");
            for (user_id,) in users {
                // Do not clobber a running manual job (single job per user).
                if let Ok(job) = store.get(user_id, &format!("user-{user_id}-backup")).await
                    && job.error.is_none()
                    && job.progress < 100
                {
                    tracing::info!(user_id = %user_id, "scheduled snapshot skipped: job in progress");
                    continue;
                }
                let options = BackupOptions {
                    include_documents: true,
                    include_files: true,
                    include_settings: true,
                    include_metadata: true,
                    memo: Some("Scheduled snapshot".into()),
                };
                let Ok(job_id) = store.create(user_id, options.clone()).await else {
                    continue;
                };
                let store2 = store.clone();
                let sql2 = sql.clone();
                let storage2 = storage.clone();
                tokio::spawn(async move {
                    // Unattended fallback: deployment passphrase if configured,
                    // else plain. Per-user passwords can't apply here — no one
                    // is holding the password at 3am.
                    run_backup(
                        store2,
                        sql2,
                        storage2,
                        user_id,
                        job_id,
                        options,
                        crate::snapcrypto::passphrase_from_env(),
                        INLINE_ZIP_TTL_MS,
                    )
                    .await;
                });
            }
        }
    });
}

/// Sweeps expired zero-target zips from the main bucket every 10 minutes.
/// Iterates all users (cheap: one user list + one prefix list per user with
/// objects; empty prefixes return fast).
pub fn spawn_inline_zip_sweeper(
    store: Arc<BackupStore>,
    sql: sqlx::PgPool,
    storage: crate::storage::S3Storage,
) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(600));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        ticker.tick().await; // skip the immediate tick
        loop {
            ticker.tick().await;
            let users: Vec<(Uuid,)> = sqlx::query_as("SELECT id FROM users")
                .fetch_all(&sql)
                .await
                .unwrap_or_default();
            let mut total = 0usize;
            for (uid,) in users {
                total += store.sweep_expired_zips(&storage, uid).await;
            }
            if total > 0 {
                tracing::info!(reclaimed = total, "inline zip sweep reclaimed expired zips");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_roundtrips_both_methods() {
        let mut a = SimpleArchive::new();
        a.add("deflated.json", br#"{"a":1}"#);
        a.add_stored("stored.bin".to_string(), &[0u8; 16]);
        let bytes = a.finish().unwrap();
        let archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        assert_eq!(archive.len(), 2);
        let names: Vec<String> = archive
            .file_names()
            .map(|s| s.unwrap().into_owned())
            .collect();
        assert!(names.contains(&"deflated.json".to_string()));
        assert!(names.contains(&"stored.bin".to_string()));
    }

    #[test]
    fn hex_sha_is_stable_64_hex() {
        let h = hex_sha(b"abc");
        assert_eq!(h.len(), 64);
        assert_eq!(h, hex_sha(b"abc"));
        assert_ne!(h, hex_sha(b"abd"));
    }
}
