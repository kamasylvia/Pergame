//! User backup jobs (Go services/backup.service.go + types/backup.go parity):
//! per-user async job, zip v1.1.0 archive (documents.json / files/ +
//! files_index.json / user_settings.json / manifest.json), upload to the
//! private backups bucket, 24h presigned download URL.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

use serde_json::json;
use tokio::sync::Mutex;

pub const BACKUP_VERSION: &str = "1.1.0";

/// Resource projection used while archiving file nodes.
pub type ResourceRow = (i64, Option<serde_json::Value>, Option<String>, Option<i64>);
const BACKUP_EXPIRY_HOURS: i64 = 24;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupOptions {
    #[serde(default)]
    pub include_documents: bool,
    #[serde(default)]
    pub include_files: bool,
    #[serde(default)]
    pub include_metadata: bool,
    #[serde(default)]
    pub include_settings: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkippedFile {
    pub id: String,
    pub name: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct BackupStats {
    pub total_nodes: i64,
    pub total_documents: i64,
    pub total_files: i64,
    pub total_size_bytes: i64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub skipped_files: Vec<SkippedFile>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupManifest {
    pub version: String,
    pub created_at: String,
    pub user_id: String,
    pub options: BackupOptions,
    pub statistics: BackupStats,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupJob {
    pub id: String,
    pub user_id: String,
    pub options: BackupOptions,
    pub progress: i32,
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
    user_jobs: Mutex<HashMap<i64, String>>,
}

fn rfc3339_now() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

impl BackupStore {
    /// Go cancelExistingBackup: flag + remove the user's previous job.
    async fn cancel_existing(&self, user_id: i64) {
        let mut user_jobs = self.user_jobs.lock().await;
        if let Some(existing) = user_jobs.remove(&user_id)
            && let Some(entry) = self.jobs.lock().await.remove(&existing)
        {
            entry.cancelled.store(true, Ordering::SeqCst);
        }
    }

    pub async fn create(
        self: &Arc<Self>,
        user_id: i64,
        options: BackupOptions,
    ) -> Result<String, crate::error::ApiError> {
        self.cancel_existing(user_id).await;

        let job_id = format!("user-{user_id}-backup");
        let now = rfc3339_now();
        let job = BackupJob {
            id: job_id.clone(),
            user_id: user_id.to_string(),
            options,
            progress: 0,
            message: "Initializing backup...".into(),
            download_url: None,
            error: None,
            created_at: now.clone(),
            updated_at: now,
            expires_at: None,
        };
        self.jobs.lock().await.insert(
            job_id.clone(),
            JobEntry {
                job,
                cancelled: Arc::new(AtomicBool::new(false)),
            },
        );
        self.user_jobs.lock().await.insert(user_id, job_id.clone());
        Ok(job_id)
    }

    pub(crate) async fn update<F: FnOnce(&mut BackupJob)>(&self, job_id: &str, f: F) {
        let mut jobs = self.jobs.lock().await;
        if let Some(entry) = jobs.get_mut(job_id) {
            f(&mut entry.job);
            entry.job.updated_at = rfc3339_now();
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
        user_id: i64,
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

    pub async fn cancel(&self, user_id: i64, job_id: &str) -> Result<(), crate::error::ApiError> {
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
}

/// Placeholder-path parity: Go fails fast when the storage client is nil.
pub async fn run_backup_no_storage(store: Arc<BackupStore>, user_id: i64, job_id: String) {
    let _ = user_id;
    store
        .update(&job_id, |j| {
            j.progress = 0;
            j.message = "minio client not initialized".into();
            j.error = Some("minio client not initialized".into());
        })
        .await;
}

/// Job phases (Go run() flow): collect → assemble zip → upload → presign.
pub async fn run_backup(
    store: Arc<BackupStore>,
    sql: sqlx::PgPool,
    storage: crate::storage::S3Storage,
    user_id: i64,
    job_id: String,
    options: BackupOptions,
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

    if store.is_cancelled(&job_id) {
        return;
    }
    progress!(5, "Preparing backup...");

    let mut stats = BackupStats::default();
    let mut file_index: Vec<serde_json::Value> = vec![];
    let mut archive = SimpleArchive::new();

    // ---------------- documents.json ----------------
    if options.include_documents {
        let rows: Vec<crate::models::NodeRow> = match sqlx::query_as::<_, crate::models::NodeRow>(
            "SELECT id, user_id, parent_id, name, description, tags, role, color, icon, thumbnail, \
             theme, accessibility, access, display, \"order\", content, content_compiled, size, \
             metadata, created_timestamp, updated_timestamp FROM nodes WHERE user_id = $1",
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
        progress!(40, "Packaging documents...");
        archive.add(
            "documents.json",
            &serde_json::to_vec_pretty(&documents).unwrap_or_default(),
        );
    }
    if store.is_cancelled(&job_id) {
        return;
    }

    // ---------------- files/ + files_index.json ----------------
    if options.include_files {
        let resources: Vec<ResourceRow> = sqlx::query_as(
            r#"SELECT id, metadata, content, size FROM nodes
                   WHERE user_id = $1 AND role = 4"#,
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
            file_index.push(json!({
                "id": res_id.to_string(),
                "path": format!("files/{res_id}{ext}"),
                "original_path": original,
                "size": size,
            }));
            archive.add_stored(format!("files/{res_id}{ext}"), &bytes);
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
        archive.add(
            "user_settings.json",
            &serde_json::to_vec_pretty(&settings).unwrap_or_default(),
        );
    }

    // ---------------- manifest.json + files_index.json ----------------
    let manifest = BackupManifest {
        version: BACKUP_VERSION.into(),
        created_at: rfc3339_now(),
        user_id: user_id.to_string(),
        options,
        statistics: stats.clone(),
    };
    progress!(85, "Creating manifest...");
    archive.add(
        "manifest.json",
        &serde_json::to_vec_pretty(&manifest).unwrap_or_default(),
    );
    if options.include_files {
        archive.add(
            "files_index.json",
            &serde_json::to_vec_pretty(&file_index).unwrap_or_default(),
        );
    }

    if store.is_cancelled(&job_id) {
        return;
    }

    // ---------------- upload + presign ----------------
    progress!(90, "Uploading backup...");
    let bytes = match archive.finish() {
        Ok(b) => b,
        Err(_) => fail!("Failed to finalize backup archive"),
    };
    let object_name = format!("{user_id}/backup.zip");
    let now_parts = time::OffsetDateTime::now_utc();
    let filename = format!(
        "pergame-backup-{:04}-{:02}-{:02}_{:02}-{:02}-{:02}.zip",
        now_parts.year(),
        u8::from(now_parts.month()),
        now_parts.day(),
        now_parts.hour(),
        now_parts.minute(),
        now_parts.second(),
    );
    if let Err(e) = storage.put_backup_object(&object_name, bytes).await {
        fail!(format!("Failed to upload backup: {e}"));
    }
    let download_url = match storage.presign_backup_get(&object_name, &filename).await {
        Ok(url) => url,
        Err(e) => fail!(format!("Failed to generate download URL: {e}")),
    };

    let expires_at = time::OffsetDateTime::now_utc()
        .checked_add(time::Duration::hours(BACKUP_EXPIRY_HOURS))
        .map(|t| {
            t.format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_default()
        });
    store
        .update(&job_id, |j| {
            j.progress = 100;
            j.message = "Backup completed successfully.".into();
            j.download_url = Some(download_url);
            j.expires_at = expires_at;
        })
        .await;
}

/// Minimal in-memory zip assembler (stored/deflated entries).
struct SimpleArchive {
    writer: Option<zip::ZipWriter<std::io::Cursor<Vec<u8>>>>,
}

impl SimpleArchive {
    fn new() -> Self {
        Self {
            writer: Some(zip::ZipWriter::new(std::io::Cursor::new(Vec::new()))),
        }
    }

    fn add(&mut self, name: &str, bytes: &[u8]) {
        self.add_with(name, bytes, zip::CompressionMethod::Deflated);
    }

    fn add_stored(&mut self, name: String, bytes: &[u8]) {
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

    fn finish(mut self) -> Result<Vec<u8>, zip::result::ZipError> {
        let writer = self.writer.take().expect("archive writer");
        let cursor = writer.finish()?;
        Ok(cursor.into_inner())
    }
}
