//! Local disk cache for attachment bytes (issue: read latency parity with a
//! kernel-side asset cache).
//!
//! Object storage is the source of truth; this layer keeps a bounded copy of
//! immutable blobs on the backend's own disk so a second view is a file read
//! instead of an S3 round trip. Only content-addressed objects are cached —
//! their bytes can never change, so no invalidation is needed. Legacy
//! `{user}/{key}` objects are skipped (they can be overwritten).
//!
//! Configuration:
//! - `ATTACHMENT_CACHE_DIR`: cache root; unset/empty disables the cache.
//! - `ATTACHMENT_CACHE_MB`: byte budget (default 2048, `0` disables).
//!
//! Eviction is LRU by access time; hits refresh it. A hit never touches S3.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::Mutex;
use uuid::Uuid;

/// Default budget when a cache directory is configured.
const DEFAULT_CACHE_MB: u64 = 2048;

/// Resolved cache configuration.
#[derive(Debug, Clone)]
pub struct DiskCache {
    root: PathBuf,
    limit_bytes: u64,
}

impl DiskCache {
    /// Build from the environment; `None` when disabled.
    pub fn from_env() -> Option<Self> {
        let dir = std::env::var("ATTACHMENT_CACHE_DIR").ok()?;
        let dir = dir.trim();
        if dir.is_empty() {
            return None;
        }
        let mb = std::env::var("ATTACHMENT_CACHE_MB")
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .unwrap_or(DEFAULT_CACHE_MB);
        if mb == 0 {
            return None;
        }
        Some(Self {
            root: PathBuf::from(dir),
            limit_bytes: mb * 1024 * 1024,
        })
    }

    /// Cache root (created lazily on first write).
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// File holding one blob (`blobs/<sha256>` layout mirrors the bucket key).
    pub fn path_for(&self, sha: &str) -> PathBuf {
        self.root.join("blobs").join(sha)
    }

    /// Existing cache entry, if any.
    pub async fn get(&self, sha: &str) -> Option<PathBuf> {
        let path = self.path_for(sha);
        let metadata = tokio::fs::metadata(&path).await.ok()?;
        if !metadata.is_file() || metadata.len() == 0 {
            return None;
        }
        // Refresh the LRU stamp (best-effort: a read-only FS still serves).
        if let Ok(file) = tokio::fs::OpenOptions::new().read(true).open(&path).await {
            let times = tokio::fs::FileTimes::new().set_accessed(std::time::SystemTime::now());
            let _ = file.set_times(times).await;
        }
        Some(path)
    }

    /// Store bytes atomically (tmp file + rename) and enforce the budget.
    pub async fn put(&self, sha: &str, bytes: &[u8]) -> std::io::Result<()> {
        if bytes.is_empty() || bytes.len() as u64 > self.limit_bytes {
            return Ok(());
        }
        let path = self.path_for(sha);
        let parent = path.parent().unwrap_or(&self.root).to_path_buf();
        tokio::fs::create_dir_all(&parent).await?;
        let tmp = parent.join(format!(".{sha}.{}.tmp", Uuid::now_v7()));
        tokio::fs::write(&tmp, bytes).await?;
        tokio::fs::rename(&tmp, &path).await?;
        let _ = self.enforce_limit().await;
        Ok(())
    }

    /// Delete least-recently-used entries until the total fits the budget.
    pub async fn enforce_limit(&self) -> std::io::Result<u64> {
        let mut entries: Vec<(PathBuf, u64, std::time::SystemTime)> = Vec::new();
        let mut total: u64 = 0;
        let mut dir = match tokio::fs::read_dir(self.root.join("blobs")).await {
            Ok(dir) => dir,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(err) => return Err(err),
        };
        while let Some(entry) = dir.next_entry().await? {
            let metadata = match entry.metadata().await {
                Ok(metadata) if metadata.is_file() => metadata,
                _ => continue,
            };
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            total += metadata.len();
            entries.push((
                entry.path(),
                metadata.len(),
                metadata
                    .accessed()
                    .or_else(|_| metadata.modified())
                    .unwrap_or(std::time::UNIX_EPOCH),
            ));
        }
        if total <= self.limit_bytes {
            return Ok(0);
        }
        entries.sort_by_key(|(_, _, atime)| *atime);
        let mut removed = 0u64;
        for (path, size, _) in entries {
            if total <= self.limit_bytes {
                break;
            }
            if tokio::fs::remove_file(&path).await.is_ok() {
                total = total.saturating_sub(size);
                removed += 1;
            }
        }
        Ok(removed)
    }
}

/// One in-flight download per blob: concurrent viewers of the same new
/// attachment wait for a single S3 fetch instead of stampeding it.
#[derive(Default)]
pub struct SingleFlight {
    locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
}

impl SingleFlight {
    /// Lock the key; the guard is released when dropped.
    pub async fn lock(&self, key: &str) -> tokio::sync::OwnedMutexGuard<()> {
        let mutex = {
            let mut locks = self.locks.lock().await;
            locks.entry(key.to_string()).or_default().clone()
        };
        mutex.lock_owned().await
    }

    /// Drop the map entry once nobody else is waiting (keeps the map bounded).
    pub async fn release(&self, key: &str) {
        let mut locks = self.locks.lock().await;
        if let Some(entry) = locks.get(key)
            && Arc::strong_count(entry) == 1
        {
            locks.remove(key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cache_in(dir: &Path, limit_mb: u64) -> DiskCache {
        DiskCache {
            root: dir.to_path_buf(),
            limit_bytes: limit_mb * 1024 * 1024,
        }
    }

    #[tokio::test]
    async fn stores_and_reads_a_blob() {
        let dir = std::env::temp_dir().join(format!("pergame-cache-{}", Uuid::now_v7()));
        let cache = cache_in(&dir, 4);
        cache.put("abc", b"hello").await.expect("put");
        assert_eq!(cache.get("abc").await, Some(cache.path_for("abc")));
        assert_eq!(
            tokio::fs::read(cache.path_for("abc")).await.unwrap(),
            b"hello"
        );
        assert!(cache.get("missing").await.is_none());
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn evicts_least_recently_used_first() {
        let dir = std::env::temp_dir().join(format!("pergame-cache-{}", Uuid::now_v7()));
        let cache = cache_in(&dir, 1);
        let chunk = vec![0u8; 700 * 1024];
        cache.put("old", &chunk).await.expect("first");
        // Age the first entry so the eviction order is deterministic.
        let old_path = cache.path_for("old");
        let file = tokio::fs::OpenOptions::new()
            .read(true)
            .open(&old_path)
            .await
            .unwrap();
        file.set_times(
            tokio::fs::FileTimes::new().set_accessed(
                std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1),
            ),
        )
        .await
        .unwrap();
        cache.put("new", &chunk).await.expect("second");
        assert!(cache.get("new").await.is_some(), "newest entry survives");
        assert!(cache.get("old").await.is_none(), "oldest entry evicted");
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn oversized_blobs_are_not_cached() {
        let dir = std::env::temp_dir().join(format!("pergame-cache-{}", Uuid::now_v7()));
        let cache = cache_in(&dir, 1);
        cache
            .put("big", &vec![0u8; 2 * 1024 * 1024])
            .await
            .expect("put");
        assert!(cache.get("big").await.is_none());
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }
}
