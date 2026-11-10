//! Backup-target configuration (spec backup-restore.md §13.4): DB-backed
//! target records with credential masking, fingerprint-based identity dedup
//! and the delete/edit gates that enforce the orphan-snapshot invariant
//! (fork kernel/backup deletecheck.go/editCheck parity).

use serde::Serialize;

use super::pg::PgTarget;
use super::s3::S3Target;
use super::webdav::WebdavTarget;
use super::{Fingerprint, PgStoreParams, S3StoreParams, TargetKind, WebdavStoreParams};
use crate::error::ApiError;

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct BackupTargetRow {
    pub id: uuid::Uuid,
    pub kind: String,
    pub name: String,
    pub endpoint: String,
    pub bucket: String,
    pub prefix: String,
    pub access_key: String,
    pub secret_key: String,
    pub secure: bool,
    pub region: String,
    pub provider: String,
    pub keep_last: i32,
    pub max_age_days: i32,
    pub schedule: String,
    pub enabled: bool,
    pub users: Vec<i64>,
    pub last_result: String,
    pub created_at: i64,
    pub updated_at: i64,
    /// Object-namespace scope inside the target prefix (siyuan #15 §1).
    /// Empty = default scope. Immutable after creation.
    pub namespace: String,
}

/// Joins a target prefix with its namespace into the effective store prefix.
/// Kept as one place so every kind scopes identically.
fn effective_prefix(prefix: &str, namespace: &str) -> String {
    let p = prefix.trim().trim_matches('/');
    let n = namespace.trim().trim_matches('/');
    match (p.is_empty(), n.is_empty()) {
        (true, true) => String::new(),
        (false, true) => p.to_string(),
        (true, false) => n.to_string(),
        (false, false) => format!("{p}/{n}"),
    }
}

/// Kind-normalized connection parameters.
pub enum TargetParams {
    S3(S3StoreParams),
    WebDav(WebdavStoreParams),
    Pg(PgStoreParams),
}

impl BackupTargetRow {
    /// Store prefix for this row: configured prefix plus namespace scope.
    pub fn scoped_prefix(&self) -> String {
        effective_prefix(&self.prefix, &self.namespace)
    }

    pub fn s3_params(&self) -> S3StoreParams {
        S3StoreParams {
            endpoint: self.endpoint.clone(),
            bucket: self.bucket.clone(),
            prefix: self.scoped_prefix(),
            access_key: self.access_key.clone(),
            secret_key: self.secret_key.clone(),
            secure: self.secure,
            region: self.region.clone(),
            provider: self.provider.clone(),
        }
    }

    pub fn webdav_params(&self) -> WebdavStoreParams {
        WebdavStoreParams {
            url: self.endpoint.clone(),
            username: self.access_key.clone(),
            password: self.secret_key.clone(),
            prefix: self.scoped_prefix(),
        }
    }

    pub fn pg_params(&self) -> PgStoreParams {
        // endpoint = host[:port], bucket = schema, access = user,
        // secret = password, secure = sslmode require (fork pg.go DSN).
        // PG has no prefix concept, so the namespace becomes an extra key
        // segment inside the table (see PgStoreParams::key_namespace).
        PgStoreParams {
            dsn: format!(
                "postgresql://{}:{}@{}/{}?sslmode={}",
                self.access_key,
                self.secret_key,
                self.endpoint,
                self.bucket,
                if self.secure { "require" } else { "prefer" }
            ),
            schema: self.bucket.clone(),
            namespace: self.namespace.trim().trim_matches('/').to_string(),
        }
    }

    pub fn kind(&self) -> Result<TargetKind, ApiError> {
        match self.kind.as_str() {
            "s3" => Ok(TargetKind::S3),
            "webdav" => Ok(TargetKind::WebDav),
            "pg" => Ok(TargetKind::Pg),
            other => Err(ApiError::BadRequest(format!(
                "unknown target kind: {other}"
            ))),
        }
    }

    /// Builds the concrete connected store handle for this row.
    pub async fn connect(&self) -> Result<TargetConn, ApiError> {
        Ok(match self.kind()? {
            TargetKind::S3 => TargetConn::S3 {
                store: S3Target::new(&self.s3_params()),
                fingerprint: self.config_fingerprint()?,
            },
            TargetKind::WebDav => TargetConn::WebDav {
                store: WebdavTarget::new(&self.webdav_params()),
                fingerprint: self.config_fingerprint()?,
            },
            TargetKind::Pg => TargetConn::Pg {
                store: PgTarget::new(&self.pg_params()).await?,
                fingerprint: self.config_fingerprint()?,
            },
        })
    }

    /// Config fingerprint without any connectivity (identity dedup on save).
    pub fn config_fingerprint(&self) -> Result<Fingerprint, ApiError> {
        Ok(match self.kind()? {
            TargetKind::S3 => super::s3::fingerprint(&self.s3_params()),
            TargetKind::WebDav => Fingerprint(super::webdav_identity(
                &self.endpoint,
                "", // unreachable-but-empty is allowed; write-time claim pins it
            )),
            TargetKind::Pg => Fingerprint(super::pg_identity(
                &self.endpoint, // endpoint doubles as the declared identity hint
                &self.bucket,
                &self.bucket,
            )),
        })
    }
}

/// Connected target handle: concrete store + its probe/fingerprint inputs.
pub enum TargetConn {
    S3 {
        store: S3Target,
        fingerprint: Fingerprint,
    },
    WebDav {
        store: WebdavTarget,
        fingerprint: Fingerprint,
    },
    Pg {
        store: PgTarget,
        fingerprint: Fingerprint,
    },
}

pub struct ProbeOutcome {
    pub reachable: bool,
    pub snapshot_objects: i64,
    pub fingerprint: Option<Fingerprint>,
    /// Owner marker read from the scope (siyuan #15 §3). None = reachable but
    /// never claimed, or unreachable.
    pub owner_instance: Option<String>,
    /// True when the marker names this deployment.
    pub owned_by_me: bool,
}

fn is_snapshot_key(key: &str) -> bool {
    // Directory placeholder objects (S3 providers like QuObjects materialise
    // `prefix/dir/` as a zero-byte key) are not snapshot payload; counting
    // them makes the delete gate refuse forever after an empty().
    if key.ends_with('/') {
        return false;
    }
    key.contains("/manifests/") || key.contains("/blobs") || key.contains("refs/latest")
}

impl TargetConn {
    /// Prefix argument for `list`: empty for stores that scope internally
    /// (S3/WebDAV apply their configured prefix), the row prefix for PG
    /// (which has no prefix concept).
    pub fn list_prefix<'a>(&self, row_prefix: &'a str) -> &'a str {
        match self {
            Self::S3 { .. } | Self::WebDav { .. } => "",
            Self::Pg { .. } => row_prefix,
        }
    }

    pub fn fingerprint(&self) -> Fingerprint {
        match self {
            Self::S3 { fingerprint, .. } => fingerprint.clone(),
            Self::WebDav { fingerprint, .. } => fingerprint.clone(),
            Self::Pg { fingerprint, .. } => fingerprint.clone(),
        }
    }

    /// Probe reachability + count snapshot-payload objects + fingerprint.
    /// Unreachable targets report reachable=false with no fingerprint.
    pub async fn probe(&self, prefix: &str) -> ProbeOutcome {
        let unreachable = ProbeOutcome {
            reachable: false,
            snapshot_objects: -1,
            fingerprint: None,
            owner_instance: None,
            owned_by_me: false,
        };
        let store_result = match self {
            Self::S3 { store, .. } => store.probe().await,
            Self::WebDav { store, .. } => store.list(prefix).await.map(|_| ()),
            Self::Pg { store, .. } => store.probe().await,
        };
        if let Err(e) = store_result {
            tracing::warn!(error = %e, "backup target probe failed");
            return unreachable;
        }
        // The concrete stores already scope by their configured prefix, so
        // the probe must not add it again (double prefix => zero objects and
        // a delete gate that wrongly passes).
        let objects = match self.list(self.list_prefix(prefix)).await {
            Ok(v) => v,
            Err(_) => return unreachable,
        };
        let snapshot_objects = objects
            .iter()
            .filter(|(k, _, _)| is_snapshot_key(k))
            .count() as i64;
        let owner = super::owner::read_owner(self).await.ok().flatten();
        let mine = super::owner::instance_id();
        let owned_by_me = owner
            .as_ref()
            .is_some_and(|o| o.instance_id == mine);
        ProbeOutcome {
            reachable: true,
            snapshot_objects,
            fingerprint: Some(self.fingerprint()),
            owner_instance: owner.map(|o| o.instance_id),
            owned_by_me,
        }
    }

    /// Byte-replicates entries (path, bytes) into this target, skipping
    /// objects that already exist (content-addressed dedup). WebDAV paths
    /// get their parent collections MKCOL'd first.
    pub async fn replicate(
        &self,
        entries: Vec<(String, Vec<u8>)>,
    ) -> Result<(usize, usize), ApiError> {
        let mut copied = 0usize;
        let mut skipped = 0usize;
        for (path, bytes) in entries {
            let exists = match self {
                Self::S3 { store, .. } => store.has(&path).await?,
                Self::WebDav { store, .. } => store.has(&path).await?,
                Self::Pg { store, .. } => store.has(&path).await?,
            };
            if exists {
                skipped += 1;
                continue;
            }
            match self {
                Self::S3 { store, .. } => store.put(&path, bytes).await?,
                Self::WebDav { store, .. } => store.put(&path, bytes).await?,
                Self::Pg { store, .. } => store.put(&path, bytes).await?,
            }
            copied += 1;
        }
        Ok((copied, skipped))
    }

    /// Writes one object (used for control-plane markers such as refs/owner).
    pub async fn put(&self, path: &str, bytes: Vec<u8>) -> Result<(), ApiError> {
        match self {
            Self::S3 { store, .. } => store.put(path, bytes).await,
            Self::WebDav { store, .. } => store.put(path, bytes).await,
            Self::Pg { store, .. } => store.put(path, bytes).await,
        }
    }

    pub async fn get(&self, path: &str) -> Result<Vec<u8>, ApiError> {
        match self {
            Self::S3 { store, .. } => store.get(path).await,
            Self::WebDav { store, .. } => store.get(path).await,
            Self::Pg { store, .. } => store.get(path).await,
        }
    }

    pub async fn delete(&self, path: &str) -> Result<bool, ApiError> {
        match self {
            Self::S3 { store, .. } => store.delete(path).await,
            Self::WebDav { store, .. } => store.delete(path).await,
            Self::Pg { store, .. } => store.delete(path).await,
        }
    }

    /// Lists objects under a prefix (key, size, mtime_ms).
    pub async fn list(&self, prefix: &str) -> Result<Vec<(String, i64, i64)>, ApiError> {
        match self {
            Self::S3 { store, .. } => store.list(prefix).await,
            Self::WebDav { store, .. } => store.list(prefix).await,
            Self::Pg { store, .. } => store.list(prefix).await,
        }
    }
}
