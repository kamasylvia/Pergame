//! Job registry and inline-zip storage for user backups.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::Mutex;
use uuid::Uuid;

use super::{BackupJob, BackupOptions, rfc3339_now};

struct JobEntry {
    job: BackupJob,
    cancelled: Arc<AtomicBool>,
}

#[derive(Default)]
/// Per-user async job registry (create/poll/cancel + inline zips).
pub struct BackupStore {
    jobs: Mutex<HashMap<String, JobEntry>>,
    user_jobs: Mutex<HashMap<Uuid, String>>,
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

    /// Start a job for one user (cancels their previous job first).
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
    pub(crate) fn is_cancelled(&self, job_id: &str) -> bool {
        match self.jobs.try_lock() {
            Ok(jobs) => match jobs.get(job_id) {
                Some(entry) => entry.cancelled.load(Ordering::SeqCst),
                None => true,
            },
            Err(_) => false,
        }
    }

    /// Fetch one job (owner-only; others see NotFound).
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

    /// Cancel one job (owner-only; sets the flag the runner polls).
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
