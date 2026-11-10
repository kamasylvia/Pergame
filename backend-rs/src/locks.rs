//! Restore/backup locking and the workspace read-only window
//! (spec backup-restore.md §14 / SiYuan fork §4 parity).
//!
//! Semantics copied from the fork, translated to this app's shape:
//! - Backups are *shared*: many may run concurrently, but the same target
//!   never twice (the caller checks conflicts before starting).
//! - Restores are *exclusive*: while one runs, no backup, restore, snapshot
//!   deletion or target emptying may start.
//! - While either lock is held the API is read-only: every workspace-mutating
//!   route (anything under /api that is not a read) answers 423 Locked, so the
//!   frontend can switch its editor to read-only and reload afterwards.
//!
//! The lock is process-wide; a single backend instance owns the workspace.

use std::sync::Arc;

use tokio::sync::RwLock;

#[derive(Debug, Default)]
pub struct WorkspaceLocks {
    /// Shared guard: one permit per running backup (refcounted by clones).
    backup_permits: RwLock<usize>,
    /// Exclusive restore flag.
    restoring: RwLock<bool>,
}

impl WorkspaceLocks {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Enters a shared (backup) lock; returns a guard that releases on drop.
    pub async fn acquire_backup(self: &Arc<Self>) -> BackupGuard {
        *self.backup_permits.write().await += 1;
        BackupGuard {
            locks: self.clone(),
        }
    }

    /// Enters the exclusive (restore) lock, waiting for backups to drain.
    pub async fn acquire_restore(self: &Arc<Self>) -> Result<RestoreGuard, LockError> {
        {
            let mut restoring = self.restoring.write().await;
            if *restoring {
                return Err(LockError::RestoreInFlight);
            }
            *restoring = true;
        }
        // Wait for in-flight backups (bounded; the caller's restore is the
        // operation the user explicitly asked for).
        for _ in 0..120 {
            if *self.backup_permits.read().await == 0 {
                return Ok(RestoreGuard {
                    locks: self.clone(),
                });
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
        *self.restoring.write().await = false;
        Err(LockError::BackupBusy)
    }

    /// True when the workspace must be treated as read-only.
    pub async fn is_read_only(&self) -> bool {
        *self.restoring.read().await || *self.backup_permits.read().await > 0
    }

    /// Snapshot of the current lock state for the status endpoint.
    pub async fn snapshot(&self) -> LockSnapshot {
        LockSnapshot {
            backups_in_flight: *self.backup_permits.read().await,
            restore_in_flight: *self.restoring.read().await,
            read_only: self.is_read_only().await,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LockSnapshot {
    pub backups_in_flight: usize,
    pub restore_in_flight: bool,
    pub read_only: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum LockError {
    RestoreInFlight,
    BackupBusy,
}

pub struct BackupGuard {
    locks: Arc<WorkspaceLocks>,
}

impl Drop for BackupGuard {
    fn drop(&mut self) {
        let locks = self.locks.clone();
        tokio::spawn(async move {
            let mut permits = locks.backup_permits.write().await;
            *permits = permits.saturating_sub(1);
        });
    }
}

pub struct RestoreGuard {
    locks: Arc<WorkspaceLocks>,
}

impl Drop for RestoreGuard {
    fn drop(&mut self) {
        let locks = self.locks.clone();
        tokio::spawn(async move {
            *locks.restoring.write().await = false;
        });
    }
}

// ---------------- read-only middleware ----------------

/// Paths that must keep working while the workspace is read-only: the lock is
/// meant to stop *content* writes, not the backup/restore machinery itself
/// (which would deadlock the very operation that set the lock).
fn is_exempt(path: &str) -> bool {
    path.starts_with("/api/backup")
        || path.starts_with("/api/admin/dr-backup")
        || path == "/api/health"
        || path.starts_with("/api/auth")
        || path.starts_with("/api/users/@me")
        || path == "/api/admin/dr-backup/status"
}

fn is_read_method(method: &axum::http::Method) -> bool {
    matches!(
        *method,
        axum::http::Method::GET | axum::http::Method::HEAD | axum::http::Method::OPTIONS
    )
}

/// Process-wide lock registry, set once at startup so the middleware can be
/// state-free (axum's `from_fn_with_state` requires the exact router state
/// type, which is awkward with a nested Router).
static LOCKS: std::sync::OnceLock<Arc<WorkspaceLocks>> = std::sync::OnceLock::new();

pub fn install(locks: Arc<WorkspaceLocks>) {
    let _ = LOCKS.set(locks);
}

pub fn global() -> Arc<WorkspaceLocks> {
    LOCKS.get().cloned().unwrap_or_else(WorkspaceLocks::new)
}

/// axum middleware: 423 Locked for workspace writes while a backup or restore
/// is running, so the editor can switch to read-only instead of failing.
pub async fn read_only_middleware(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let path = req.uri().path().to_string();
    if is_read_method(req.method()) || is_exempt(&path) || !global().is_read_only().await {
        return next.run(req).await;
    }
    use axum::response::IntoResponse;
    (
        axum::http::StatusCode::LOCKED,
        axum::Json(serde_json::json!({
            "message": "Workspace is read-only while a backup or restore is running.",
            "status": "error",
            "reason": "backup_in_progress",
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn backup_is_shared_restore_is_exclusive() {
        let locks = WorkspaceLocks::new();
        assert!(!locks.is_read_only().await);

        let b1 = locks.acquire_backup().await;
        let b2 = locks.acquire_backup().await;
        assert!(
            locks.is_read_only().await,
            "backups make the workspace read-only"
        );
        drop(b1);
        drop(b2);
        // Guards release asynchronously; give the tasks a tick.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(!locks.is_read_only().await);

        let restore = locks.acquire_restore().await.expect("first restore wins");
        assert!(locks.is_read_only().await);
        let second = locks.acquire_restore().await;
        assert!(matches!(second, Err(LockError::RestoreInFlight)));
        drop(restore);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(!locks.is_read_only().await);
    }
}
