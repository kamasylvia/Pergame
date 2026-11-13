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
/// Process-wide backup/restore lock registry (one instance owns the workspace).
pub struct WorkspaceLocks {
    /// Shared guard: one permit per running backup (refcounted by clones).
    backup_permits: RwLock<usize>,
    /// Target ids currently written by in-flight backups (fork §4: the same
    /// target never runs twice — a new backup whose targets intersect this
    /// set is rejected with the conflicting ids).
    busy_targets: RwLock<std::collections::HashSet<uuid::Uuid>>,
    /// Exclusive restore flag.
    restoring: RwLock<bool>,
}

impl WorkspaceLocks {
    /// Empty registry (no backups, no restore).
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Enters a shared (backup) lock over `targets`; returns a guard that
    /// releases on drop. Rejected when any target is already being written
    /// (the caller lists the conflict and retries later).
    pub async fn acquire_backup(
        self: &Arc<Self>,
        targets: &[uuid::Uuid],
    ) -> Result<BackupGuard, LockError> {
        {
            let busy = self.busy_targets.read().await;
            let conflict: Vec<uuid::Uuid> = targets
                .iter()
                .filter(|t| busy.contains(t))
                .copied()
                .collect();
            if !conflict.is_empty() {
                return Err(LockError::TargetBusy(conflict));
            }
        }
        *self.backup_permits.write().await += 1;
        self.busy_targets
            .write()
            .await
            .extend(targets.iter().copied());
        Ok(BackupGuard {
            locks: self.clone(),
            targets: targets.to_vec(),
        })
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

    /// True while a restore holds the exclusive lock. Snapshot deletion and
    /// target emptying refuse while this holds (fork §4: restores are
    /// exclusive over deletes too).
    pub async fn is_restoring(&self) -> bool {
        *self.restoring.read().await
    }

    /// Snapshot of the current lock state for the status endpoint.
    pub async fn snapshot(&self) -> LockSnapshot {
        LockSnapshot {
            backups_in_flight: *self.backup_permits.read().await,
            restore_in_flight: *self.restoring.read().await,
            read_only: self.is_read_only().await,
            busy_targets: self.busy_targets.read().await.iter().copied().collect(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
/// Status-endpoint projection of the lock state.
pub struct LockSnapshot {
    pub backups_in_flight: usize,
    pub restore_in_flight: bool,
    pub read_only: bool,
    pub busy_targets: Vec<uuid::Uuid>,
}

#[derive(Debug, Clone)]
/// Why a lock acquisition was refused.
pub enum LockError {
    RestoreInFlight,
    BackupBusy,
    TargetBusy(Vec<uuid::Uuid>),
}

/// Held by one running backup; releases its targets on drop.
pub struct BackupGuard {
    locks: Arc<WorkspaceLocks>,
    targets: Vec<uuid::Uuid>,
}

impl Drop for BackupGuard {
    fn drop(&mut self) {
        let locks = self.locks.clone();
        let targets = std::mem::take(&mut self.targets);
        tokio::spawn(async move {
            let mut permits = locks.backup_permits.write().await;
            *permits = permits.saturating_sub(1);
            let mut busy = locks.busy_targets.write().await;
            for t in targets {
                busy.remove(&t);
            }
        });
    }
}

/// Held by the running restore; clears the flag on drop.
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
    // Fork §4 parity: setting writes are NOT exempt — a setting changed
    // mid-backup would silently miss the snapshot. GETs pass via
    // is_read_method, so dropping /api/users/@me only blocks writes.
    //
    // /api/mcp is exempt at this layer because every MCP call is a POST,
    // reads included; the MCP service gates its *write tools* on the same
    // lock itself (spec features/mcp-server.md), so the read-only window
    // still stops content changes while staying legible to an agent.
    path.starts_with("/api/backup")
        || path.starts_with("/api/admin/backup")
        || path == "/api/health"
        || path.starts_with("/api/auth")
        || path.starts_with("/api/mcp")
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

/// Install the registry for the read-only middleware (once, at startup).
pub fn install(locks: Arc<WorkspaceLocks>) {
    let _ = LOCKS.set(locks);
}

/// Registry handle (fresh empty one before `install`).
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

        let t1 = uuid::Uuid::now_v7();
        let t2 = uuid::Uuid::now_v7();
        let b1 = locks.acquire_backup(&[t1]).await.expect("first wins");
        // Same target twice is rejected (fork §4); disjoint targets run free.
        let clash = locks.acquire_backup(&[t1]).await;
        assert!(matches!(clash, Err(LockError::TargetBusy(_))));
        let b2 = locks.acquire_backup(&[t2]).await.expect("disjoint ok");
        assert!(
            locks.is_read_only().await,
            "backups make the workspace read-only"
        );
        drop(b1);
        drop(b2);
        // Guards release asynchronously; give the tasks a tick.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(!locks.is_read_only().await);
        // Released targets accept new backups again.
        let b3 = locks.acquire_backup(&[t1]).await.expect("released");
        drop(b3);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let restore = locks.acquire_restore().await.expect("first restore wins");
        assert!(locks.is_read_only().await);
        assert!(locks.is_restoring().await);
        let second = locks.acquire_restore().await;
        assert!(matches!(second, Err(LockError::RestoreInFlight)));
        drop(restore);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(!locks.is_read_only().await);
        assert!(!locks.is_restoring().await);
    }

    #[tokio::test]
    async fn exempts_machinery_not_content() {
        for exempt in [
            "/api/backup",
            "/api/backup/snapshots/1/restore",
            "/api/admin/backup/targets",
            "/api/health",
            "/api/auth/refresh",
            "/api/mcp",
            "/api/mcp/sse",
        ] {
            assert!(is_exempt(exempt), "{exempt} must stay writable");
        }
        for blocked in [
            "/api/nodes",
            "/api/users/@me",
            "/api/trash/1",
            "/api/stats/users",
        ] {
            assert!(!is_exempt(blocked), "{blocked} must lock");
        }
        use axum::http::Method;
        assert!(is_read_method(&Method::GET));
        assert!(is_read_method(&Method::HEAD));
        assert!(is_read_method(&Method::OPTIONS));
        assert!(!is_read_method(&Method::POST));
        assert!(!is_read_method(&Method::PUT));
        assert!(!is_read_method(&Method::DELETE));
    }

    #[tokio::test]
    async fn snapshot_reports_state() {
        let locks = WorkspaceLocks::new();
        let snap = locks.snapshot().await;
        assert!(!snap.read_only);
        assert_eq!(snap.backups_in_flight, 0);
        assert!(!snap.restore_in_flight);
        let t = uuid::Uuid::now_v7();
        let _b = locks.acquire_backup(&[t]).await.unwrap();
        let snap = locks.snapshot().await;
        assert!(snap.read_only);
        assert_eq!(snap.backups_in_flight, 1);
        assert_eq!(snap.busy_targets, vec![t]);
    }
}
