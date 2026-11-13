//! User backup jobs (Go services/backup.service.go + types/backup.go parity):
//! per-user async job, portable zip archive (documents.json — `{nodes, links}`
//! — / files/ + files_index.json / user_settings.json / manifest.json /
//! permissions.json / invitations.json / trash.json / profile.json), written
//! straight to the user's own backup targets; zero targets = S3 inline zip
//! with user-chosen expiry, downloaded via a one-shot token link.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

mod run;
mod schedule;
mod store;

pub use run::{run_backup, run_backup_no_storage};
pub(crate) use schedule::{SimpleArchive, hex_sha};
pub use schedule::{spawn_inline_zip_sweeper, spawn_snapshot_scheduler};
pub use store::BackupStore;

/// Portable zip format version written by this backend.
pub const BACKUP_VERSION: &str = "2.0.0";

/// Snapshot storage format: v2 = manifest + content-addressed blobs
/// (`{uid}/manifests/<ts>.json` + `{uid}/blobs[-e2e]/<sha256>`), deduped
/// across snapshots; v1 = single zip (`{uid}/snapshots/<ts>.zip`), still
/// restorable/downloadable. New snapshots are always v2.
/// Snapshots kept per user; older ones are pruned after a successful backup
/// (app-side retention replacing the removed bucket lifecycle rule).
pub const SNAPSHOT_KEEP: usize = 10;

/// Resource projection used while archiving file nodes.
pub type ResourceRow = (Uuid, Option<serde_json::Value>, Option<String>, Option<i64>);

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
/// Which members a snapshot includes (documents/files/settings/…).
pub struct BackupOptions {
    #[serde(default)]
    pub include_documents: bool,
    #[serde(default)]
    pub include_files: bool,
    #[serde(default)]
    pub include_metadata: bool,
    #[serde(default)]
    pub include_settings: bool,
    /// Snapshot annotation, stored in the manifest (max 200 chars,
    /// trimmed at the API boundary).
    #[serde(default)]
    pub memo: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// One file skipped during archiving (id/name/reason).
pub struct SkippedFile {
    pub id: String,
    pub name: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
/// Archive counters (nodes/documents/links/files/bytes).
pub struct BackupStats {
    pub total_nodes: i64,
    pub total_documents: i64,
    pub total_links: i64,
    pub total_files: i64,
    pub total_size_bytes: i64,
    #[serde(default)]
    pub total_permissions: i64,
    #[serde(default)]
    pub total_invitations: i64,
    #[serde(default)]
    pub total_trashed: i64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped_files: Vec<SkippedFile>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
/// Snapshot descriptor (id/created/stats/entries), stored per target.
pub struct BackupManifest {
    pub version: String,
    pub id: String,
    pub created_at: String,
    pub user_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encryption: Option<crate::snapcrypto::EncryptionParams>,
    pub options: BackupOptions,
    pub statistics: BackupStats,
    /// Logical members (documents.json / user_settings.json): path -> blob sha.
    pub entries: Vec<ManifestEntry>,
    /// Restored file attachments: original storage key + blob sha.
    pub files: Vec<ManifestFile>,
    /// Sum of member + file byte sizes (logical snapshot size).
    pub logical_bytes: u64,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
/// One archived node entry (path/sha/size).
pub struct ManifestEntry {
    pub path: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
/// One archived file entry (node id + storage key).
pub struct ManifestFile {
    pub id: String,
    pub key: String, // user-object storage key (transformed_path)
    pub original_path: Option<String>,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
/// In-flight job state (progress/status/result zip handle).
pub struct BackupJob {
    pub id: String,
    pub user_id: String,
    pub options: BackupOptions,
    pub progress: i32,
    /// pending | processing | completed | failed — derived in update().
    pub status: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

/// Largest zero-target snapshot kept in memory (matches the upload cap).
pub const MAX_INLINE_ZIP_BYTES: usize = 256 * 1024 * 1024;

/// Zero-target zip lifetime: 1 hour, then the sweeper drops the bytes.
pub const INLINE_ZIP_TTL_MS: i64 = 3600 * 1000;

pub(crate) fn rfc3339_now() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc3339_now_is_parseable() {
        let s = rfc3339_now();
        let parsed =
            time::OffsetDateTime::parse(&s, &time::format_description::well_known::Rfc3339);
        assert!(parsed.is_ok());
    }
}
