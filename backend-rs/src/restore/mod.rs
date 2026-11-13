//! Snapshot restore engine (spec .work/features/backup-restore.md §3).
//!
//! Two entrances feed the same engine: restoring a snapshot already stored on
//! a user backup target, and restoring an uploaded zip. Strategy is replace:
//! the caller's nodes (cascading links/permissions/invitations) are removed
//! inside a transaction, then rebuilt from documents.json; links pointing at
//! missing nodes are skipped and reported.

mod apply;
mod members;
mod plan;
mod portable;

pub use apply::restore_from_zip;
pub use plan::{RestorePlan, assemble_snapshot_zip, plan_from_zip};
pub use portable::{
    PORTABLE_FORMAT, PORTABLE_VERSION, PortableMeta, build_portable_zip_in_memory,
    export_portable_zip_from_conn, is_portable_zip, portable_to_v1_zip,
};

use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

/// zip safety caps (spec §3): at most 5000 entries, 100MB per entry.
pub(crate) const MAX_ENTRIES: usize = 5000;
pub(crate) const MAX_ENTRY_BYTES: u64 = 100 * 1024 * 1024;

#[derive(Debug, Default, Serialize)]
/// What a restore rebuilt/skipped (counts + per-kind skip lists).
pub struct RestoreReport {
    pub nodes: usize,
    pub links: usize,
    pub links_skipped: Vec<String>,
    pub files: usize,
    pub files_skipped: Vec<String>,
    pub settings: bool,
    pub permissions: usize,
    pub permissions_skipped: Vec<String>,
    pub invitations: usize,
    pub invitations_skipped: Vec<String>,
    pub trashed: usize,
    pub trashed_skipped: Vec<String>,
    pub profile: bool,
    pub warnings: Vec<String>,
}

/// Highest accepted archive format (semver-ish major.minor pair).
pub(crate) const MAX_FORMAT: (u32, u32) = (2, 0); // v2 archives (manifest+blobs assembled) and legacy 1.x

pub(crate) fn as_i64(v: Option<&Value>) -> Option<i64> {
    v.and_then(|v| {
        v.as_i64()
            .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
    })
}

pub(crate) fn as_uuid(v: Option<&Value>) -> Option<Uuid> {
    v.and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
}

pub(crate) fn as_opt_uuid(v: Option<&Value>) -> Option<Uuid> {
    match v {
        Some(Value::Null) | None => None,
        other => as_uuid(other),
    }
}

pub(crate) fn as_opt_i64(v: Option<&Value>) -> Option<i64> {
    match v {
        Some(Value::Null) | None => None,
        other => as_i64(other),
    }
}

pub(crate) fn as_string(v: Option<&Value>) -> Option<String> {
    v.and_then(|v| v.as_str()).map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_coercions() {
        assert_eq!(as_i64(Some(&json!(42))), Some(42));
        assert_eq!(as_i64(Some(&json!("42"))), Some(42));
        assert_eq!(as_i64(Some(&json!("nope"))), None);
        assert_eq!(as_i64(None), None);
        let id = Uuid::now_v7();
        assert_eq!(as_uuid(Some(&json!(id.to_string()))), Some(id));
        assert_eq!(as_uuid(Some(&json!("nope"))), None);
        assert_eq!(as_opt_uuid(Some(&json!(null))), None);
        assert_eq!(as_opt_uuid(None), None);
        assert_eq!(as_opt_i64(Some(&json!(null))), None);
        assert_eq!(as_opt_i64(Some(&json!(7))), Some(7));
        assert_eq!(as_string(Some(&json!("x"))), Some("x".to_string()));
        assert_eq!(as_string(Some(&json!(1))), None);
    }
}
