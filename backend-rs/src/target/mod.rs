//! Fan-out target stores (spec backup-restore.md §13): one immutable
//! object-key/value store per DR destination. Object keys mirror the local
//! backup bucket layout (manifests/, blobs[-e2e]/, refs/) so fan-out is a
//! byte-level replica
#![allow(dead_code)] // consumed by fan-out + admin routes (P1, in progress) and restore-from-target needs no translation.

pub mod config;
pub mod cron;
pub mod fanout;
pub mod owner;
pub mod repo;
pub mod s3;
pub mod webdav;

use serde::Serialize;

use crate::error::ApiError;

/// Normalized physical identity of a target store. Two configs describing
/// the same physical storage (host vs IP, trailing slashes, case) must
/// produce equal fingerprints — that is what delete/edit gating compares.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Fingerprint(pub String);

#[derive(Debug, Clone)]
/// Supported fan-out store kinds.
pub enum TargetKind {
    S3,
    WebDav,
}

impl TargetKind {
    /// Wire name (`s3` / `webdav`).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::S3 => "s3",
            Self::WebDav => "webdav",
        }
    }
}

/// One object row from `list` (key, size, mtime_ms).
pub type ObjectRow = (String, i64, i64);

/// Async-trait-free object store: implemented per kind, driven with plain
/// async fns on the concrete types (we only ever hold one of three kinds).
/// S3 connection parameters (parsed from one target row).
pub struct S3StoreParams {
    pub endpoint: String,
    pub bucket: String,
    pub prefix: String,
    pub access_key: String,
    pub secret_key: String,
    pub secure: bool,
    pub region: String,
    pub provider: String,
}

/// WebDAV connection parameters (parsed from one target row).
pub struct WebdavStoreParams {
    pub url: String,
    pub username: String,
    pub password: String,
    pub prefix: String,
}

/// Normalized identity string pieces (joined into a Fingerprint upstream).
pub fn s3_identity(endpoint: &str, bucket: &str) -> String {
    format!(
        "{}|{}",
        normalize_endpoint(endpoint).to_lowercase(),
        bucket.trim().to_lowercase()
    )
}

/// Normalized `url|hash` identity (empty hash = not yet claimed).
pub fn webdav_identity(url: &str, claimed_hash: &str) -> String {
    if claimed_hash.is_empty() {
        String::new() // never written: no identity yet (gating treats as clear)
    } else {
        format!(
            "{}|{}",
            normalize_endpoint(url).to_lowercase(),
            claimed_hash
        )
    }
}

fn normalize_endpoint(endpoint: &str) -> String {
    let e = endpoint.trim();
    let e = e
        .strip_prefix("https://")
        .or_else(|| e.strip_prefix("http://"))
        .unwrap_or(e);
    e.trim_end_matches('/').to_string()
}

pub(crate) fn internal(e: impl std::fmt::Display) -> ApiError {
    let msg = e.to_string();
    tracing::error!(error = %msg, "backup target operation failed");
    // Surface the detail to the admin UI: "internal error" alone is useless
    // for a misconfigured target (wrong bucket policy, clock skew, creds).
    ApiError::BadRequest(format!("target store error: {msg}"))
}

/// Same, but preserving a store-level NotFound distinction.
pub(crate) fn not_found(what: &str) -> ApiError {
    ApiError::NotFound(format!("target object not found: {what}"))
}

pub(crate) fn join_prefix(prefix: &str, rest: &str) -> String {
    let p = prefix.trim().trim_matches('/');
    match (p.is_empty(), rest.starts_with('/')) {
        (true, _) => rest.to_string(),
        (false, true) => format!("{p}{rest}"),
        (false, false) => format!("{p}/{rest}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s3_identity_normalizes_endpoint_and_bucket() {
        let a = s3_identity("https://s3.example.com/", "MyBucket");
        let b = s3_identity("http://s3.example.com", "mybucket");
        assert_eq!(a, b);
        assert!(a.contains('|'));
        // Different bucket = different identity.
        assert_ne!(a, s3_identity("https://s3.example.com", "other"));
    }

    #[test]
    fn webdav_identity_empty_hash_means_unclaimed() {
        assert_eq!(webdav_identity("https://dav.example/x", ""), "");
        let id = webdav_identity("https://dav.example/x/", "abc123");
        assert_eq!(id, "dav.example/x|abc123");
    }

    #[test]
    fn join_prefix_handles_slashes() {
        assert_eq!(join_prefix("", "a/b"), "a/b");
        assert_eq!(join_prefix("p", "a/b"), "p/a/b");
        assert_eq!(join_prefix("p/", "/a/b"), "p/a/b");
        assert_eq!(join_prefix("/p/", "a"), "p/a");
    }

    #[test]
    fn internal_error_surfaces_detail() {
        let e = internal("disk gone");
        assert!(matches!(e, ApiError::BadRequest(m) if m.contains("disk gone")));
        let nf = not_found("k");
        assert!(matches!(nf, ApiError::NotFound(_)));
    }
}
