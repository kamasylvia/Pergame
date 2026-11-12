//! User backup jobs (Go services/backup.service.go + types/backup.go parity):
//! per-user async job, portable zip archive (documents.json — `{nodes, links}`
//! — / files/ + files_index.json / user_settings.json / manifest.json /
//! permissions.json / invitations.json / trash.json / profile.json), written
//! straight to the user's own backup targets; zero targets = S3 inline zip
//! with user-chosen expiry, downloaded via a one-shot token link.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

use serde_json::json;
use tokio::sync::Mutex;
use uuid::Uuid;

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
pub struct SkippedFile {
    pub id: String,
    pub name: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
pub struct ManifestEntry {
    pub path: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct ManifestFile {
    pub id: String,
    pub key: String, // user-object storage key (transformed_path)
    pub original_path: Option<String>,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
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

struct JobEntry {
    job: BackupJob,
    cancelled: Arc<AtomicBool>,
}

#[derive(Default)]
pub struct BackupStore {
    jobs: Mutex<HashMap<String, JobEntry>>,
    user_jobs: Mutex<HashMap<Uuid, String>>,
}

/// Largest zero-target snapshot kept in memory (matches the upload cap).
pub const MAX_INLINE_ZIP_BYTES: usize = 256 * 1024 * 1024;

/// Zero-target zip lifetime: 1 hour, then the sweeper drops the bytes.
pub const INLINE_ZIP_TTL_MS: i64 = 3600 * 1000;

fn rfc3339_now() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

impl BackupStore {
    /// Go cancelExistingBackup: flag + remove the user's previous job.
    async fn cancel_existing(&self, user_id: Uuid) {
        let mut user_jobs = self.user_jobs.lock().await;
        if let Some(existing) = user_jobs.remove(&user_id)
            && let Some(entry) = self.jobs.lock().await.remove(&existing)
        {
            entry.cancelled.store(true, Ordering::SeqCst);
        }
    }

    pub async fn create(
        self: &Arc<Self>,
        user_id: Uuid,
        options: BackupOptions,
    ) -> Result<String, crate::error::ApiError> {
        self.cancel_existing(user_id).await;
        let job_id = format!("user-{user_id}-backup");
        self.register(&job_id, user_id, options).await;
        Ok(job_id)
    }

    /// Inserts a job row under an explicit id without touching the user's
    /// current job (for inline runners like the pre-restore safety snapshot,
    /// which must not cancel anything and is never polled).
    pub async fn register(&self, job_id: &str, user_id: Uuid, options: BackupOptions) {
        let now = rfc3339_now();
        let job = BackupJob {
            id: job_id.to_string(),
            user_id: user_id.to_string(),
            options,
            progress: 0,
            status: "pending".into(),
            message: "Initializing backup...".into(),
            download_url: None,
            error: None,
            created_at: now.clone(),
            updated_at: now,
            expires_at: None,
        };
        self.jobs.lock().await.insert(
            job_id.to_string(),
            JobEntry {
                job,
                cancelled: Arc::new(AtomicBool::new(false)),
            },
        );
    }

    pub(crate) async fn update<F: FnOnce(&mut BackupJob)>(&self, job_id: &str, f: F) {
        let mut jobs = self.jobs.lock().await;
        if let Some(entry) = jobs.get_mut(job_id) {
            f(&mut entry.job);
            entry.job.updated_at = rfc3339_now();
            // Single derivation point: every status read sees a consistent value.
            entry.job.status = if entry.job.error.is_some() {
                "failed".into()
            } else if entry.job.progress >= 100 {
                "completed".into()
            } else if entry.job.progress > 0 {
                "processing".into()
            } else {
                "pending".into()
            };
        }
    }

    /// Best-effort cancellation check; a removed job counts as cancelled.
    fn is_cancelled(&self, job_id: &str) -> bool {
        match self.jobs.try_lock() {
            Ok(jobs) => match jobs.get(job_id) {
                Some(entry) => entry.cancelled.load(Ordering::SeqCst),
                None => true,
            },
            Err(_) => false,
        }
    }

    pub async fn get(
        &self,
        user_id: Uuid,
        job_id: &str,
    ) -> Result<BackupJob, crate::error::ApiError> {
        use crate::error::ApiError;
        let jobs = self.jobs.lock().await;
        let entry = jobs
            .get(job_id)
            .ok_or(ApiError::NotFound("backup job not found".into()))?;
        if entry.job.user_id != user_id.to_string() {
            return Err(ApiError::NotFound(
                "unauthorized access to backup job".into(),
            ));
        }
        Ok(entry.job.clone())
    }

    pub async fn cancel(&self, user_id: Uuid, job_id: &str) -> Result<(), crate::error::ApiError> {
        use crate::error::ApiError;
        let mut jobs = self.jobs.lock().await;
        let entry = jobs
            .get_mut(job_id)
            .ok_or(ApiError::NotFound("backup job not found".into()))?;
        if entry.job.user_id != user_id.to_string() {
            return Err(ApiError::NotFound(
                "unauthorized access to backup job".into(),
            ));
        }
        entry.cancelled.store(true, Ordering::SeqCst);
        let job_id_owned = job_id.to_string();
        self.user_jobs.lock().await.remove(&user_id);
        drop(jobs);
        self.jobs.lock().await.remove(&job_id_owned);
        Ok(())
    }

    /// Stores a zero-target snapshot zip in the main bucket
    /// (`{uid}/inline-zips/{job_id}.zip` + `.json` expiry sidecar).
    /// Returns (expiry_ms, download_token). The token authorizes one
    /// download without a session cookie (links outlive access tokens).
    pub async fn put_inline_zip(
        &self,
        storage: &crate::storage::S3Storage,
        user_id: Uuid,
        job_id: &str,
        bytes: Vec<u8>,
        ttl_ms: i64,
    ) -> (i64, String) {
        use rand::RngExt;
        let expires_at = crate::auth::service::now_ms() + ttl_ms;
        let token: String = rand::rng()
            .sample_iter(&rand::distr::Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();
        let base = format!("inline-zips/{job_id}");
        if storage
            .put_user_object(
                user_id,
                &format!("{base}.zip"),
                bytes,
                Some("application/zip"),
            )
            .await
            .is_err()
        {
            return (0, String::new());
        }
        let _ = storage
            .put_user_object(
                user_id,
                &format!("{base}.json"),
                serde_json::to_vec(&serde_json::json!({
                    "expires_at": expires_at,
                    "token": token,
                }))
                .unwrap_or_default(),
                Some("application/json"),
            )
            .await;
        (expires_at, token)
    }

    /// Takes (and removes) a stored zip. Authorized by the download token
    /// (no session needed — links outlive access tokens). The token is a
    /// 32-char secret in the sidecar; wrong/missing token = 404 (no
    /// distinction from expired, to avoid user-id enumeration).
    /// Expired entries behave as missing (and are cleaned up).
    pub async fn take_inline_zip(
        &self,
        storage: &crate::storage::S3Storage,
        user_id: Uuid,
        job_id: &str,
        token: &str,
    ) -> Result<Vec<u8>, crate::error::ApiError> {
        use crate::error::ApiError;
        let not_found = || ApiError::NotFound("snapshot zip expired or already downloaded".into());
        let base = format!("inline-zips/{job_id}");
        let sidecar: serde_json::Value = storage
            .get_user_object(user_id, &format!("{base}.json"))
            .await
            .ok()
            .and_then(|raw| serde_json::from_slice(&raw).ok())
            .ok_or_else(not_found)?;
        let token_ok = sidecar
            .get("token")
            .and_then(|v| v.as_str())
            .is_some_and(|t| !t.is_empty() && t == token);
        if !token_ok {
            return Err(not_found());
        }
        let expired = sidecar
            .get("expires_at")
            .and_then(|n| n.as_i64())
            .is_some_and(|exp| exp <= crate::auth::service::now_ms());
        let bytes = storage
            .get_user_object(user_id, &format!("{base}.zip"))
            .await
            .map_err(|_| ApiError::NotFound("snapshot zip expired or already downloaded".into()))?;
        // Single-download semantics: remove both objects after a good read.
        // Expired zips are removed too (lazy sweep on access).
        storage
            .delete_user_object(user_id, &format!("{base}.zip"))
            .await;
        storage
            .delete_user_object(user_id, &format!("{base}.json"))
            .await;
        if expired {
            return Err(ApiError::NotFound("snapshot zip expired".into()));
        }
        Ok(bytes)
    }

    /// Drops expired inline zips for one user (called by the sweeper).
    /// Returns the number of zips reclaimed.
    pub async fn sweep_expired_zips(
        &self,
        storage: &crate::storage::S3Storage,
        user_id: Uuid,
    ) -> usize {
        let now = crate::auth::service::now_ms();
        let Ok(objects) = storage.list_user_prefix(user_id, "inline-zips").await else {
            return 0;
        };
        let mut reclaimed = 0usize;
        for (key, _, _) in objects {
            if !key.ends_with(".json") {
                continue;
            }
            let rel = match key.strip_prefix(&format!("{user_id}/")) {
                Some(r) => r.to_string(),
                None => continue,
            };
            let expired = storage
                .get_user_object(user_id, &rel)
                .await
                .ok()
                .and_then(|raw| serde_json::from_slice::<serde_json::Value>(&raw).ok())
                .and_then(|v| v.get("expires_at").and_then(|n| n.as_i64()))
                .is_some_and(|exp| exp <= now);
            if expired {
                let zip_rel = rel.trim_end_matches(".json").to_string() + ".zip";
                storage.delete_user_object(user_id, &zip_rel).await;
                storage.delete_user_object(user_id, &rel).await;
                reclaimed += 1;
            }
        }
        reclaimed
    }
}

/// Placeholder-path parity: Go fails fast when the storage client is nil.
pub async fn run_backup_no_storage(store: Arc<BackupStore>, user_id: Uuid, job_id: String) {
    let _ = user_id;
    store
        .update(&job_id, |j| {
            j.progress = 0;
            j.message = "minio client not initialized".into();
            j.error = Some("minio client not initialized".into());
        })
        .await;
}

/// Soft-link row projection for documents.json (spec note-links.md §2: the
/// backup carries the links table; a future restore rebuilds rows and skips +
/// reports ones pointing at missing nodes). UUIDs render as text to
/// keep the string-on-the-wire contract.
#[derive(Debug, sqlx::FromRow, Serialize)]
struct NodeLinkBackup {
    id: String,
    parent_id: String,
    node_id: String,
    created_timestamp: i64,
    order: i32,
}

/// Job phases (Go run() flow): collect → assemble zip → upload → presign.
/// `passphrase`: caller-supplied snapshot password (fork §2 semantics). None
/// = plain snapshot. The passphrase is never stored; unattended schedulers
/// pass the deployment fallback (`passphrase_from_env`), which is usually
/// unset — an unconfigured scheduler simply produces plain snapshots.
/// `zip_ttl_ms`: zero-target zip lifetime in the main bucket (user-chosen
/// 1h/1d/1w; unattended callers pass the 1h default).
#[allow(clippy::too_many_arguments)] // backup entrypoint: handles + ids + knobs, splitting buys nothing
pub async fn run_backup(
    store: Arc<BackupStore>,
    sql: sqlx::PgPool,
    storage: crate::storage::S3Storage,
    user_id: Uuid,
    job_id: String,
    options: BackupOptions,
    passphrase: Option<String>,
    zip_ttl_ms: i64,
) {
    macro_rules! progress {
        ($p:expr, $msg:expr) => {
            store
                .update(&job_id, |j| {
                    j.progress = $p;
                    j.message = $msg.into();
                })
                .await;
        };
    }
    macro_rules! fail {
        ($msg:expr) => {{
            let msg = $msg.to_string();
            store
                .update(&job_id, |j| {
                    j.progress = 0;
                    j.message = msg.clone();
                    j.error = Some(msg);
                })
                .await;
            return;
        }};
    }

    // Shared lock over this run's targets (fork §4): concurrent backups
    // allowed, but the same target never twice. The workspace is read-only
    // for the duration (released when the guard drops at function exit).
    // Targets resolve before locking: a collection-phase failure must not
    // hold anybody else's targets.
    let target_ids: Vec<Uuid> =
        match crate::target::repo::list_targets_for_user(&sql, user_id).await {
            Ok(t) => t.into_iter().map(|r| r.id).collect(),
            Err(_) => fail!("Failed to list backup targets"),
        };
    let _lock = match crate::locks::global().acquire_backup(&target_ids).await {
        Ok(g) => g,
        Err(crate::locks::LockError::TargetBusy(conflict)) => fail!(format!(
            "Backup target(s) busy: {}; retry when the running backup finishes",
            conflict
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )),
        Err(_) => fail!("Failed to acquire backup lock"),
    };
    if store.is_cancelled(&job_id) {
        return;
    }
    progress!(5, "Preparing backup...");

    let encryption = passphrase
        .as_ref()
        .filter(|p| !p.trim().is_empty())
        .map(|_| crate::snapcrypto::new_params());
    let passphrase = passphrase.filter(|p| !p.trim().is_empty());

    // ---------------- v2: collect members, then content-addressed blobs ----
    let mut members: Vec<(String, Vec<u8>)> = Vec::new();
    let mut files_meta: Vec<ManifestFile> = Vec::new();
    // (sha, e2e, bytes) for each attachment, uploaded in the write pass.
    let mut file_payloads: Vec<(String, bool, Vec<u8>)> = Vec::new();
    let mut stats = BackupStats::default();

    if options.include_documents {
        let rows: Vec<crate::models::NodeRow> = match sqlx::query_as::<_, crate::models::NodeRow>(
            "SELECT id, user_id, parent_id, name, description, tags, role, color, icon, thumbnail, \
             theme, accessibility, access, display, \"order\" AS node_order, content, content_compiled, size, \
             metadata, created_timestamp, updated_timestamp FROM nodes WHERE user_id = $1 \
             AND deleted_at IS NULL",
        )
        .bind(user_id)
        .fetch_all(&sql)
        .await
        {
            Ok(r) => r,
            Err(_) => fail!("Failed to prepare documents"),
        };
        stats.total_nodes = rows.len() as i64;
        stats.total_documents = rows.iter().filter(|r| r.role == 3).count() as i64;
        let documents: Vec<serde_json::Value> = rows
            .iter()
            .map(|r| {
                let mut v = serde_json::to_value(r).unwrap_or(serde_json::Value::Null);
                if !options.include_metadata
                    && let Some(obj) = v.as_object_mut()
                {
                    obj.remove("metadata");
                }
                v
            })
            .collect();

        // Soft links owned via the user's nodes (node ownership anchors the
        // row; parent must be one of the user's own folders by construction).
        let links: Vec<NodeLinkBackup> = match sqlx::query_as::<_, NodeLinkBackup>(
            "SELECT nl.id::text, nl.parent_id::text, nl.node_id::text, nl.created_timestamp, nl.\"order\" \
             FROM node_links nl JOIN nodes n ON n.id = nl.node_id \
             WHERE n.user_id = $1 AND n.deleted_at IS NULL ORDER BY nl.id",
        )
        .bind(user_id)
        .fetch_all(&sql)
        .await
        {
            Ok(r) => r,
            Err(_) => fail!("Failed to prepare document links"),
        };
        stats.total_links = links.len() as i64;
        progress!(40, "Packaging documents...");
        members.push((
            "documents.json".into(),
            serde_json::to_vec_pretty(&json!({
                "nodes": documents,
                "links": links,
            }))
            .unwrap_or_default(),
        ));
    }
    // ---------------- permissions + invitations (self-owned only) --------
    // Permissions store (node_id, grantee_username, permission): usernames,
    // not user ids — grantee ids would dangle cross-instance and waste
    // bytes. Restore resolves usernames back to local users, skipping
    // unknown ones. Invitations are self-contained rows (the code is the
    // identity), exported whole.
    if options.include_documents {
        #[derive(Debug, sqlx::FromRow, serde::Serialize)]
        struct PermissionBackup {
            node_id: String,
            grantee: String,
            permission: i32,
            created_timestamp: i64,
        }
        let permissions: Vec<PermissionBackup> = sqlx::query_as::<_, PermissionBackup>(
            "SELECT p.node_id::text AS node_id, u.username AS grantee, p.permission, \
             p.created_timestamp \
             FROM permissions p \
             JOIN nodes n ON n.id = p.node_id \
             JOIN users u ON u.id = p.user_id \
             WHERE n.user_id = $1 AND n.deleted_at IS NULL ORDER BY p.node_id",
        )
        .bind(user_id)
        .fetch_all(&sql)
        .await
        .unwrap_or_default();
        stats.total_permissions = permissions.len() as i64;
        members.push((
            "permissions.json".into(),
            serde_json::to_vec_pretty(&permissions).unwrap_or_default(),
        ));

        #[derive(Debug, sqlx::FromRow, serde::Serialize)]
        struct InvitationBackup {
            invitation_code: String,
            permission_level: i32,
            node_id: String,
            created_timestamp: i64,
        }
        let invitations: Vec<InvitationBackup> = sqlx::query_as::<_, InvitationBackup>(
            "SELECT ni.invitation_code, ni.permission_level, ni.node_id::text AS node_id, \
             ni.created_timestamp \
             FROM node_invitations ni JOIN nodes n ON n.id = ni.node_id \
             WHERE n.user_id = $1 AND n.deleted_at IS NULL ORDER BY ni.node_id",
        )
        .bind(user_id)
        .fetch_all(&sql)
        .await
        .unwrap_or_default();
        stats.total_invitations = invitations.len() as i64;
        members.push((
            "invitations.json".into(),
            serde_json::to_vec_pretty(&invitations).unwrap_or_default(),
        ));
    }
    if store.is_cancelled(&job_id) {
        return;
    }

    // ---------------- files (each attachment becomes a blob) ----------------
    if options.include_files {
        let resources: Vec<ResourceRow> = sqlx::query_as(
            r#"SELECT id, metadata, content, size FROM nodes
                   WHERE user_id = $1 AND role = 4 AND deleted_at IS NULL"#,
        )
        .bind(user_id)
        .fetch_all(&sql)
        .await
        .unwrap_or_default();
        stats.total_files = resources.len() as i64;
        progress!(60, "Archiving files...");
        for (res_id, metadata, original, size) in &resources {
            if store.is_cancelled(&job_id) {
                return;
            }
            let ext = original
                .as_deref()
                .and_then(|n| n.rsplit_once('.').map(|(_, e)| format!(".{e}")))
                .unwrap_or_default();
            let transformed = metadata
                .as_ref()
                .and_then(|m| m.get("transformed_path"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("{res_id}{ext}"));
            let bytes = match storage.get_user_object(user_id, &transformed).await {
                Ok(b) => b,
                Err(_) => {
                    stats.skipped_files.push(SkippedFile {
                        id: res_id.to_string(),
                        name: original.clone().unwrap_or_default(),
                        reason: "object not found in storage".into(),
                    });
                    continue;
                }
            };
            let sha = hex_sha(&bytes);
            let (e2e, payload) = match (&encryption, &passphrase) {
                (Some(params), Some(pass)) => {
                    match crate::snapcrypto::encrypt(params, pass, &bytes) {
                        Ok(b) => (true, b),
                        Err(e) => {
                            fail!(format!("Failed to encrypt blob: {e}"));
                        }
                    }
                }
                _ => (false, bytes.clone()),
            };
            // Held in memory: direct-write uploads everything to the user's
            // targets in one pass below (dedup happens per target there).
            file_payloads.push((sha.clone(), e2e, payload));
            files_meta.push(ManifestFile {
                id: res_id.to_string(),
                key: transformed,
                original_path: original.clone(),
                sha256: sha,
                size: (*size).unwrap_or(bytes.len() as i64) as u64,
            });
        }
    }

    // ---------------- user_settings.json ----------------
    if options.include_settings
        && let Some((g, e, a)) =
            sqlx::query_as::<_, (serde_json::Value, serde_json::Value, serde_json::Value)>(
                "SELECT general, editor, advanced FROM user_settings WHERE user_id = $1",
            )
            .bind(user_id)
            .fetch_optional(&sql)
            .await
            .ok()
            .flatten()
    {
        let settings = serde_json::json!({
            "user_id": user_id.to_string(),
            "general": g, "editor": e, "advanced": a,
        });
        members.push((
            "user_settings.json".into(),
            serde_json::to_vec_pretty(&settings).unwrap_or_default(),
        ));
    }

    // ---------------- trash.json (soft-deleted nodes, merge on restore) --
    // Live documents.json stays live-only; trashed rows ride separately so
    // restore can replace live data while merging trash (never clobbering
    // a node the user already recovered).
    if options.include_documents {
        #[derive(Debug, sqlx::FromRow, serde::Serialize)]
        struct TrashNodeBackup {
            id: Uuid,
            user_id: Uuid,
            parent_id: Option<Uuid>,
            name: String,
            description: Option<String>,
            tags: Option<String>,
            role: i32,
            color: Option<i32>,
            icon: Option<String>,
            thumbnail: Option<String>,
            theme: Option<String>,
            accessibility: Option<i32>,
            access: i32,
            display: Option<i32>,
            #[serde(rename = "order")]
            #[sqlx(rename = "node_order")]
            node_order: Option<i32>,
            content: Option<String>,
            content_compiled: Option<String>,
            size: Option<i64>,
            metadata: Option<serde_json::Value>,
            created_timestamp: i64,
            updated_timestamp: i64,
            deleted_at: i64,
        }
        let trashed: Vec<TrashNodeBackup> = sqlx::query_as::<_, TrashNodeBackup>(
            "SELECT id, user_id, parent_id, name, description, tags, role, color, icon, thumbnail, \
             theme, accessibility, access, display, \"order\" AS node_order, content, content_compiled, size, \
             metadata, created_timestamp, updated_timestamp, deleted_at FROM nodes WHERE user_id = $1 \
             AND deleted_at IS NOT NULL",
        )
        .bind(user_id)
        .fetch_all(&sql)
        .await
        .unwrap_or_default();
        stats.total_trashed = trashed.len() as i64;
        if !trashed.is_empty() {
            members.push((
                "trash.json".into(),
                serde_json::to_vec_pretty(&json!({ "nodes": trashed })).unwrap_or_default(),
            ));
        }
    }

    // ---------------- profile.json (display fields only) ------------------
    // No credentials, no role/type: restoring a profile must never change
    // who you are or what you can do — only how your name looks.
    if options.include_settings {
        #[derive(Debug, sqlx::FromRow, serde::Serialize)]
        struct ProfileBackup {
            username: String,
            firstname: Option<String>,
            lastname: Option<String>,
            avatar: Option<String>,
            email: Option<String>,
            created_timestamp: i64,
        }
        if let Ok(Some(profile)) = sqlx::query_as::<_, ProfileBackup>(
            "SELECT username, firstname, lastname, avatar, email, created_timestamp \
                 FROM users WHERE id = $1",
        )
        .bind(user_id)
        .fetch_optional(&sql)
        .await
        {
            members.push((
                "profile.json".into(),
                serde_json::to_vec_pretty(&profile).unwrap_or_default(),
            ));
        }
    }
    let snapshot_id = crate::auth::service::now_ms();
    let manifest = BackupManifest {
        version: BACKUP_VERSION.into(),
        id: snapshot_id.to_string(),
        created_at: rfc3339_now(),
        user_id: user_id.to_string(),
        memo: options.memo.clone(),
        encryption: encryption.clone(),
        options: options.clone(),
        statistics: stats.clone(),
        entries: members
            .iter()
            .map(|(path, bytes)| ManifestEntry {
                path: path.clone(),
                sha256: hex_sha(bytes),
                size: bytes.len() as u64,
            })
            .collect(),
        files: files_meta.clone(),
        logical_bytes: members.iter().map(|(_, b)| b.len() as u64).sum::<u64>()
            + files_meta.iter().map(|f| f.size).sum::<u64>(),
    };
    progress!(85, "Writing blobs...");

    // Encrypt member blobs once; every target receives identical bytes.
    let mut member_payloads: Vec<(String, bool, Vec<u8>)> = Vec::with_capacity(members.len());
    for (_path, bytes) in &members {
        let sha = hex_sha(bytes);
        let (e2e, payload) = match (&encryption, &passphrase) {
            (Some(params), Some(pass)) => match crate::snapcrypto::encrypt(params, pass, bytes) {
                Ok(b) => (true, b),
                Err(e) => {
                    fail!(format!("Failed to encrypt blob: {e}"));
                }
            },
            _ => (false, bytes.clone()),
        };
        member_payloads.push((sha, e2e, payload));
    }

    // Direct-write: the snapshot lands on the user's own targets, never on
    // the service bucket. Zero targets = fork §4 "save zip locally": the
    // portable set is held in memory for one download, nothing is persisted.
    // (Target ids come from the lock-time query above — same list we hold.)
    if target_ids.is_empty() {
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).unwrap_or_default();
        match crate::restore::build_portable_zip_in_memory(
            &manifest,
            &manifest_bytes,
            &member_payloads,
            &file_payloads,
        ) {
            Ok(zip) if zip.len() <= MAX_INLINE_ZIP_BYTES => {
                let (expires_ms, token) = store
                    .put_inline_zip(&storage, user_id, &job_id, zip, zip_ttl_ms)
                    .await;
                store
                    .update(&job_id, |j| {
                        j.progress = 100;
                        j.message =
                            "Snapshot packaged for download (no targets configured).".into();
                        j.download_url =
                            Some(format!("/api/backup/{job_id}/download?token={token}"));
                        j.expires_at = Some(expires_ms.to_string());
                    })
                    .await;
                return;
            }
            Ok(zip) => fail!(format!(
                "Snapshot is {} MB with no targets configured (limit {} MB); configure a backup target first",
                zip.len() / 1_048_576,
                MAX_INLINE_ZIP_BYTES / 1_048_576
            )),
            Err(e) => fail!(format!("Failed to package snapshot: {e}")),
        }
    }
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).unwrap_or_default();
    let results = crate::target::fanout::write_snapshot_to_targets(
        &sql,
        user_id,
        &manifest,
        &manifest_bytes,
        &member_payloads,
        &file_payloads,
    )
    .await;
    let mut failed = 0usize;
    for r in &results {
        if let Some(e) = &r.error {
            failed += 1;
            tracing::warn!(target = %r.target, error = %e, "direct-write failed");
        } else {
            tracing::info!(target = %r.target, copied = r.copied, skipped = r.skipped, "direct-write done");
        }
    }
    if failed == results.len() && !results.is_empty() {
        fail!(format!(
            "All {} target(s) failed: {}",
            results.len(),
            results
                .iter()
                .filter_map(|r| r.error.clone())
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    let ok = results.len() - failed;
    store
        .update(&job_id, |j| {
            j.progress = 100;
            j.message = if failed == 0 {
                format!("Snapshot completed successfully ({} target(s)).", ok)
            } else {
                format!("Snapshot completed with {failed} failed target(s) ({ok} ok).")
            };
            // download_url intentionally unset: snapshots download through
            // the authenticated /backup/snapshots/:id/download endpoint.
        })
        .await;
}

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
/// SNAPSHOT_INTERVAL_HOURS (env, 0/absent = disabled) and snapshots each user
/// that has data, memoed "Scheduled snapshot". Retention/GC from the normal
/// backup path applies, and a user's in-flight manual job is never clobbered.
pub fn spawn_snapshot_scheduler(
    store: Arc<BackupStore>,
    sql: sqlx::PgPool,
    storage: crate::storage::S3Storage,
) {
    let hours: u64 = match std::env::var("SNAPSHOT_INTERVAL_HOURS")
        .ok()
        .and_then(|v| v.trim().parse().ok())
    {
        Some(h) if h > 0 => h,
        _ => {
            tracing::info!("SNAPSHOT_INTERVAL_HOURS unset/0: scheduled snapshots disabled");
            return;
        }
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
