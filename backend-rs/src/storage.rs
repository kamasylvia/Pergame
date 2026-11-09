//! S3 storage layer (Go app/minio.go + minio service parity).
//!
//! Environment (2026-11-07 rename, issue #2): S3_ENDPOINT / S3_ACCESSKEY /
//! S3_SECRETKEY / S3_BUCKET / S3_PUBLIC_URL / S3_SECURE / S3_INSECURE_TLS /
//! S3_CA_PATH.
//!
//! TLS semantics vs the Go client: S3_CA_PATH loads a PEM bundle as extra
//! trust roots on top of the platform store via the smithy TLS context.
//! S3_INSECURE_TLS stays unimplemented — aws-smithy-http-client offers no
//! certificate-verifier bypass, so verification cannot be turned off (the
//! env var logs a warning when set). Endpoint scheme normalization, the
//! localhost→HTTP fallback and the `{bucket}-backups` lifecycle mirror the
//! Go behavior.

use aws_sdk_s3::config::{Builder as S3ConfigBuilder, Credentials};
use aws_smithy_http_client::tls::{TlsContext, TrustStore};
use aws_smithy_runtime_api::client::http::SharedHttpClient;

const BACKUP_PRESIGN_SECONDS: u64 = 24 * 3600;
use aws_sdk_s3::Client;
use aws_sdk_s3::operation::create_bucket::CreateBucketOutput;

use crate::error::ApiError;

#[derive(Debug, Clone)]
pub struct S3Storage {
    pub client: Client,
    pub bucket: String,
    pub backup_bucket: String,
}

fn env(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(v) if !v.is_empty() => Some(v),
        _ => None,
    }
}

/// Go parity quirk: S3_SECURE defaults to true when unset; localhost/127.*
/// endpoints always downgrade to HTTP.
fn resolve_endpoint(endpoint_raw: &str, secure_env: Option<&str>) -> String {
    if endpoint_raw.contains("://") {
        return endpoint_raw.to_string();
    }
    let mut secure = !matches!(secure_env, Some(v) if v != "true");
    if endpoint_raw.starts_with("localhost") || endpoint_raw.starts_with("127.") {
        secure = false;
    }
    let scheme = if secure { "https" } else { "http" };
    format!("{scheme}://{endpoint_raw}")
}

pub fn parse_env() -> Result<S3Storage, String> {
    let endpoint = env("S3_ENDPOINT").ok_or("S3 NOT CONFIGURED")?;
    let access = env("S3_ACCESSKEY").ok_or("S3 NOT CONFIGURED")?;
    let secret = env("S3_SECRETKEY").ok_or("S3 NOT CONFIGURED")?;
    let bucket = env("S3_BUCKET").unwrap_or_else(|| "pergame".into());
    let secure_env = std::env::var("S3_SECURE").ok();
    let url = resolve_endpoint(&endpoint, secure_env.as_deref());

    if env("S3_CA_PATH").is_some() {
        tracing::info!("S3_CA_PATH set; loading custom CA bundle into the TLS trust store");
    }
    if env("S3_INSECURE_TLS").is_some() {
        tracing::warn!(
            "S3_INSECURE_TLS is not honored: aws-smithy-http-client has no certificate-verifier \
             bypass, certificate verification stays ON"
        );
    }

    let mut s3cfg = S3ConfigBuilder::new()
        .behavior_version(crate_state_version())
        .endpoint_url(url)
        .region(aws_sdk_s3::config::Region::new("us-east-1"))
        .credentials_provider(Credentials::new(access, secret, None, None, "S3Storage"))
        .force_path_style(true);

    if let Some(ca_path) = env("S3_CA_PATH") {
        let http_client = load_custom_ca_client(&ca_path)?;
        s3cfg = s3cfg.http_client(http_client);
    }

    let s3cfg = s3cfg.build();

    Ok(S3Storage {
        client: Client::from_conf(s3cfg),
        backup_bucket: format!("{bucket}-backups"),
        bucket,
    })
}

fn crate_state_version() -> aws_sdk_s3::config::BehaviorVersion {
    aws_sdk_s3::config::BehaviorVersion::latest()
}

/// Builds an HTTP client whose TLS context trusts the PEM bundle at `ca_path`
/// in addition to the platform roots (Go parity: S3_CA_PATH custom CA).
fn load_custom_ca_client(ca_path: &str) -> Result<SharedHttpClient, String> {
    let pem = std::fs::read_to_string(ca_path)
        .map_err(|e| format!("S3_CA_PATH: cannot read {ca_path}: {e}"))?;
    let tls = TlsContext::builder()
        .with_trust_store(TrustStore::default().with_pem_certificate(pem))
        .build()
        .map_err(|e| format!("S3_CA_PATH: invalid certificate bundle: {e}"))?;
    Ok(aws_smithy_http_client::Builder::new()
        .tls_provider(aws_smithy_http_client::tls::Provider::rustls(
            aws_smithy_http_client::tls::rustls_provider::CryptoMode::Ring,
        ))
        .tls_context(tls)
        .build_https())
}

impl S3Storage {
    /// Creates both buckets idempotently; sets the public-read policy (warn +
    /// continue on providers without PutBucketPolicy, Go parity). The backups
    /// bucket has no lifecycle rule: snapshot retention is managed by the app
    /// (keep-last-N prune, see backup.rs) instead of a blanket expiry.
    pub async fn ensure_buckets(&self) -> Result<(), ApiError> {
        self.ensure_bucket(&self.bucket).await?;
        self.set_public_policy(&self.bucket).await;
        self.ensure_bucket(&self.backup_bucket).await?;
        self.clear_backup_lifecycle(&self.backup_bucket).await;
        Ok(())
    }

    async fn ensure_bucket(&self, bucket: &str) -> Result<(), ApiError> {
        match self.client.create_bucket().bucket(bucket).send().await {
            Ok(CreateBucketOutput { .. }) => {
                tracing::info!("Successfully created {}", bucket);
                Ok(())
            }
            Err(e) => {
                let msg = e.to_string();
                // BucketAlreadyOwnedByYou / BucketAlreadyExists surface as
                // errors with those names; treat them as success.
                if msg.contains("BucketAlready") || msg.contains("409") {
                    tracing::info!("We already own {}", bucket);
                    Ok(())
                } else {
                    Err(ApiError::Internal(Box::new(std::io::Error::other(
                        format!("failed to create bucket {}: {}", bucket, msg),
                    ))))
                }
            }
        }
    }

    async fn set_public_policy(&self, bucket: &str) {
        let policy = format!(
            r#"{{"Version":"2012-10-17","Statement":[{{"Effect":"Allow","Principal":{{"AWS":["*"]}},"Action":["s3:GetObject"],"Resource":["arn:aws:s3:::{}/*"]}}]}}"#,
            bucket
        );
        match self
            .client
            .put_bucket_policy()
            .bucket(bucket)
            .policy(policy)
            .send()
            .await
        {
            Ok(_) => tracing::info!("Successfully set public bucket policy for {}", bucket),
            Err(e) => {
                tracing::warn!("Failed to set public bucket policy for {}: {}", bucket, e);
                tracing::warn!(
                    "Make sure objects in this bucket are publicly readable (configure it in your S3 provider), otherwise uploaded files will not be accessible."
                );
            }
        }
    }

    /// Previously a 24h blanket lifecycle rule deleted backups a day after
    /// upload; snapshots are now multi-version and app-pruned, so no lifecycle
    /// rule is installed. Best-effort removal for buckets that still carry the
    /// old rule from earlier bootstraps.
    async fn clear_backup_lifecycle(&self, bucket: &str) {
        let _ = self
            .client
            .delete_bucket_lifecycle()
            .bucket(bucket)
            .send()
            .await;
    }

    /// PUT object under `{user_id}/{key}`.
    pub async fn put_user_object(
        &self,
        user_id: i64,
        key: &str,
        content: Vec<u8>,
        content_type: Option<&str>,
    ) -> Result<(), ApiError> {
        let object_key = format!("{}/{}", user_id, key);
        let mut req = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(&object_key)
            .body(content.into());
        if let Some(ct) = content_type {
            req = req.content_type(ct);
        }
        req.send().await.map(|_| ()).map_err(|e| {
            tracing::error!(error = %e, %object_key, "put_object failed");
            ApiError::Internal(Box::new(std::io::Error::other("failed to store file")))
        })
    }

    /// Head-checks a blob; true when it already exists (content-addressed
    /// dedup: skip the PUT).
    pub async fn blob_exists(
        &self,
        user_id: i64,
        e2e: bool,
        sha256_hex: &str,
    ) -> Result<bool, ApiError> {
        let dir = if e2e { "blobs-e2e" } else { "blobs" };
        let key = format!("{user_id}/{dir}/{sha256_hex}");
        match self
            .client
            .head_object()
            .bucket(&self.backup_bucket)
            .key(&key)
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("404") || msg.contains("NotFound") || msg.contains("NoSuchKey") {
                    Ok(false)
                } else {
                    Err(ApiError::Internal(Box::new(std::io::Error::other(
                        format!("blob head failed: {msg}"),
                    ))))
                }
            }
        }
    }

    /// PUTs a blob under `{uid}/blobs[-e2e]/{sha256}` (caller deduped).
    pub async fn put_blob(
        &self,
        user_id: i64,
        e2e: bool,
        sha256_hex: &str,
        content: Vec<u8>,
    ) -> Result<(), ApiError> {
        let dir = if e2e { "blobs-e2e" } else { "blobs" };
        let key = format!("{user_id}/{dir}/{sha256_hex}");
        self.client
            .put_object()
            .bucket(&self.backup_bucket)
            .key(&key)
            .body(content.into())
            .send()
            .await
            .map(|_| ())
            .map_err(|e| {
                tracing::error!(error = %e, %key, "put blob failed");
                ApiError::Internal(Box::new(std::io::Error::other("failed to store blob")))
            })
    }

    /// GETs a blob by hex sha (decryption is the caller's concern).
    pub async fn get_blob(
        &self,
        user_id: i64,
        e2e: bool,
        sha256_hex: &str,
    ) -> Result<Vec<u8>, ApiError> {
        let dir = if e2e { "blobs-e2e" } else { "blobs" };
        let key = format!("{user_id}/{dir}/{sha256_hex}");
        let resp = self
            .client
            .get_object()
            .bucket(&self.backup_bucket)
            .key(&key)
            .send()
            .await
            .map_err(|_| ApiError::NotFound("blob not found".into()))?;
        let bytes = resp.body.collect().await.map_err(|e| {
            tracing::error!(error = %e, %key, "read blob failed");
            ApiError::Internal(Box::new(std::io::Error::other("failed to read blob")))
        })?;
        Ok(bytes.to_vec())
    }

    /// Lists manifest objects under `{uid}/manifests/` as (key, size, mtime_ms).
    pub async fn list_manifests(&self, user_id: i64) -> Result<Vec<(String, i64, i64)>, ApiError> {
        let prefix = format!("{user_id}/manifests/");
        let mut out = Vec::new();
        let resp = self
            .client
            .list_objects_v2()
            .bucket(&self.backup_bucket)
            .prefix(&prefix)
            .send()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "list manifests failed");
                ApiError::Internal(Box::new(std::io::Error::other("failed to list snapshots")))
            })?;
        for obj in resp.contents() {
            let Some(key) = obj.key() else { continue };
            let size = obj.size().unwrap_or(0);
            let mtime = obj
                .last_modified()
                .and_then(|t| t.to_millis().ok())
                .unwrap_or(0);
            out.push((key.to_string(), size, mtime));
        }
        Ok(out)
    }

    /// Lists all blob keys under both blob dirs (for GC reachability checks).
    pub async fn list_blob_keys(&self, user_id: i64) -> Result<Vec<String>, ApiError> {
        let mut out = Vec::new();
        for dir in ["blobs", "blobs-e2e"] {
            let prefix = format!("{user_id}/{dir}/");
            let resp = self
                .client
                .list_objects_v2()
                .bucket(&self.backup_bucket)
                .prefix(&prefix)
                .send()
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, "list blobs failed");
                    ApiError::Internal(Box::new(std::io::Error::other("failed to list blobs")))
                })?;
            for obj in resp.contents() {
                if let Some(key) = obj.key() {
                    out.push(key.to_string());
                }
            }
        }
        Ok(out)
    }

    /// Lists backup-snapshot objects under `{uid}/snapshots/` as
    /// (key, size_bytes, last_modified_epoch_ms), oldest first.
    pub async fn list_backup_snapshots(
        &self,
        user_id: i64,
    ) -> Result<Vec<(String, i64, i64)>, ApiError> {
        let prefix = format!("{user_id}/snapshots/");
        let mut out = Vec::new();
        let resp = self
            .client
            .list_objects_v2()
            .bucket(&self.backup_bucket)
            .prefix(&prefix)
            .send()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "list snapshots failed");
                ApiError::Internal(Box::new(std::io::Error::other("failed to list snapshots")))
            })?;
        for obj in resp.contents() {
            let Some(key) = obj.key() else { continue };
            let size = obj.size().unwrap_or(0);
            let mtime = obj
                .last_modified()
                .and_then(|t| t.to_millis().ok())
                .unwrap_or(0);
            out.push((key.to_string(), size, mtime));
        }
        Ok(out)
    }

    /// Reads one backup-snapshot object fully (key without bucket).
    pub async fn get_backup_object(&self, key: &str) -> Result<Vec<u8>, ApiError> {
        let resp = self
            .client
            .get_object()
            .bucket(&self.backup_bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, %key, "get backup object failed");
                ApiError::NotFound("snapshot not found".into())
            })?;
        let bytes = resp.body.collect().await.map_err(|e| {
            tracing::error!(error = %e, %key, "read backup object failed");
            ApiError::Internal(Box::new(std::io::Error::other("failed to read snapshot")))
        })?;
        Ok(bytes.to_vec())
    }

    /// Deletes one backup-snapshot object (best-effort logging on failure).
    pub async fn delete_backup_object(&self, key: &str) -> Result<(), ApiError> {
        self.client
            .delete_object()
            .bucket(&self.backup_bucket)
            .key(key)
            .send()
            .await
            .map(|_| ())
            .map_err(|e| {
                tracing::error!(error = %e, %key, "delete backup object failed");
                ApiError::Internal(Box::new(std::io::Error::other("failed to delete snapshot")))
            })
    }

    /// Deletes `{user_id}/{key}` (best-effort, Go parity: failures logged).
    pub async fn delete_user_object(&self, user_id: i64, key: &str) {
        let object_key = format!("{}/{}", user_id, key);
        if let Err(e) = self
            .client
            .delete_object()
            .bucket(&self.bucket)
            .key(&object_key)
            .send()
            .await
        {
            tracing::warn!(%object_key, error = %e, "failed to delete object");
        }
    }

    /// Deletes everything under the `{user_id}/{node_id}` prefix (fallback
    /// sweep of a resource node, Go DeleteSingleResource fallback path).
    pub async fn delete_node_prefix(&self, user_id: i64, node_id: i64) {
        let prefix = format!("{}/{}/", user_id, node_id);
        let Ok(list) = self
            .client
            .list_objects_v2()
            .bucket(&self.bucket)
            .prefix(&prefix)
            .send()
            .await
        else {
            return;
        };
        let Some(objects) = list.contents else { return };
        for obj in objects {
            let Some(key) = obj.key else { continue };
            if let Err(e) = self
                .client
                .delete_object()
                .bucket(&self.bucket)
                .key(&key)
                .send()
                .await
            {
                tracing::warn!(%key, error = %e, "failed to delete object");
            }
        }
    }

    /// GET object bytes (backup archiving).
    pub async fn get_user_object(&self, user_id: i64, key: &str) -> Result<Vec<u8>, ApiError> {
        let object_key = format!("{}/{}", user_id, key);
        let resp = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&object_key)
            .send()
            .await
            .map_err(|e| {
                tracing::warn!(%object_key, error = %e, "get_object failed");
                ApiError::BadRequest("object not found".into())
            })?;
        let bytes = resp
            .body
            .collect()
            .await
            .map_err(|e| {
                tracing::error!(%object_key, error = %e, "object body read failed");
                ApiError::BadRequest("failed to read object".into())
            })?
            .into_bytes();
        Ok(bytes.to_vec())
    }

    /// PUT into the backups bucket (v1.1.0 archive).
    pub async fn put_backup_object(&self, key: &str, content: Vec<u8>) -> Result<(), String> {
        self.client
            .put_object()
            .bucket(&self.backup_bucket)
            .key(key)
            .content_type("application/zip")
            .body(content.into())
            .send()
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// 24h presigned GET with a download filename (Go PresignedGetObject +
    /// response-content-disposition parity).
    pub async fn presign_backup_get(&self, key: &str, filename: &str) -> Result<String, String> {
        use aws_sdk_s3::presigning::PresigningConfig;
        let req = self
            .client
            .get_object()
            .bucket(&self.backup_bucket)
            .key(key)
            .response_content_disposition(format!(r#"attachment; filename="{filename}""#));
        let cfg = PresigningConfig::builder()
            .expires_in(std::time::Duration::from_secs(BACKUP_PRESIGN_SECONDS))
            .build()
            .map_err(|e| e.to_string())?;
        req.presigned(cfg)
            .await
            .map(|p| p.uri().to_string())
            .map_err(|e| e.to_string())
    }

    /// Storage cleanup for a deleted resource node: metadata path first, then
    /// prefix sweep (Go DeleteSingleResource parity).
    pub async fn delete_resource_files(
        &self,
        user_id: i64,
        node_id: i64,
        metadata: Option<&serde_json::Value>,
    ) {
        if let Some(meta) = metadata
            && let Some(path) = meta.get("transformed_path").and_then(|v| v.as_str())
            && !path.is_empty()
        {
            self.delete_user_object(user_id, path).await;
            return;
        }
        self.delete_node_prefix(user_id, node_id).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_endpoint_scheme_quirks() {
        // Go parity: unset S3_SECURE defaults to HTTPS; localhost/127.* and an
        // explicit non-true value downgrade to HTTP; full URLs pass through.
        assert_eq!(
            resolve_endpoint("s3.example.com:9000", None),
            "https://s3.example.com:9000"
        );
        assert_eq!(
            resolve_endpoint("localhost:9000", None),
            "http://localhost:9000"
        );
        assert_eq!(
            resolve_endpoint("127.0.0.1:9000", Some("true")),
            "http://127.0.0.1:9000"
        );
        assert_eq!(
            resolve_endpoint("s3.example.com:9000", Some("false")),
            "http://s3.example.com:9000"
        );
        assert_eq!(
            resolve_endpoint("https://s3.example.com", Some("false")),
            "https://s3.example.com"
        );
    }

    #[test]
    fn custom_ca_client_builds_and_bad_path_errors() {
        // PEM bytes are only validated at handshake time; the builder path must
        // succeed for any file content and fail on an unreadable path.
        let path = std::env::temp_dir().join("pergame-storage-test-ca.pem");
        std::fs::write(&path, "not a real pem").unwrap();
        assert!(load_custom_ca_client(path.to_str().unwrap()).is_ok());
        assert!(load_custom_ca_client("/nonexistent/pergame-ca.pem").is_err());
        std::fs::remove_file(&path).ok();
    }
}
