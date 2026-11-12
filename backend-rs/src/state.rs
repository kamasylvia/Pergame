use sqlx::PgPool;
use toasty::Db;

use crate::backup::BackupStore;
use crate::oidc::SharedManager;
use crate::storage::S3Storage;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    /// Toasty ORM handle (same database; schema owned by sqlx migrations).
    pub toasty: Db,
    /// S3-compatible storage (None = CDN features disabled).
    pub storage: Option<S3Storage>,
    /// OIDC provider manager (None = OIDC disabled).
    pub oidc: Option<SharedManager>,
    /// Per-user backup job store.
    pub backups: std::sync::Arc<BackupStore>,
    /// Workspace backup/restore locks (spec §14): a read-only window exists
    /// while any backup or restore is in flight.
    pub locks: std::sync::Arc<crate::locks::WorkspaceLocks>,
}
