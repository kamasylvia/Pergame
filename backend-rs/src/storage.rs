//! S3 storage layer (Go app/minio.go + minio service parity).
//!
//! Environment (2026-11-07 rename, issue #2): S3_ENDPOINT / S3_ACCESSKEY /
//! S3_SECRETKEY / S3_BUCKET / S3_PUBLIC_URL / S3_SECURE / S3_INSECURE_TLS /
//! S3_CA_PATH.
//!
//! Known deltas vs the Go client (recorded in GOAL-STATE): S3_CA_PATH custom
//! roots and S3_INSECURE_TLS verification bypass are not wired into the AWS
//! SDK's TLS stack yet — those env vars log a warning when set. Endpoint
//! scheme normalization, the localhost→HTTP fallback and the
//! `{bucket}-backups` lifecycle mirror the Go behavior.

use aws_sdk_s3::config::{Builder as S3ConfigBuilder, Credentials};

const BACKUP_PRESIGN_SECONDS: u64 = 24 * 3600;
use aws_sdk_s3::operation::create_bucket::CreateBucketOutput;
use aws_sdk_s3::Client;

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
    let bucket = env("S3_BUCKET").unwrap_or_else(|| "alexandrie".into());
    let secure_env = std::env::var("S3_SECURE").ok();
    let url = resolve_endpoint(&endpoint, secure_env.as_deref());

    if env("S3_CA_PATH").is_some() {
        tracing::warn!("S3_CA_PATH is not supported by the Rust storage client yet (custom CA not loaded)");
    }
    if env("S3_INSECURE_TLS").is_some() {
        tracing::warn!("S3_INSECURE_TLS is not supported by the Rust storage client yet (verification stays on)");
    }

    let s3cfg = S3ConfigBuilder::new()
        .behavior_version(crate_state_version())
        .endpoint_url(url)
        .region(aws_sdk_s3::config::Region::new("us-east-1"))
        .credentials_provider(Credentials::new(access, secret, None, None, "S3Storage"))
        .force_path_style(true)
        .build();

    Ok(S3Storage {
        client: Client::from_conf(s3cfg),
        backup_bucket: format!("{bucket}-backups"),
        bucket,
    })
}

fn crate_state_version() -> aws_sdk_s3::config::BehaviorVersion {
    aws_sdk_s3::config::BehaviorVersion::latest()
}

impl S3Storage {
    /// Creates both buckets idempotently; sets the public-read policy (warn +
    /// continue on providers without PutBucketPolicy, Go parity) and the
    /// 24h backup expiry lifecycle.
    pub async fn ensure_buckets(&self) -> Result<(), ApiError> {
        self.ensure_bucket(&self.bucket).await?;
        self.set_public_policy(&self.bucket).await;
        self.ensure_bucket(&self.backup_bucket).await?;
        self.set_backup_lifecycle(&self.backup_bucket).await;
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
                    Err(ApiError::Internal(Box::new(std::io::Error::other(format!(
                        "failed to create bucket {}: {}",
                        bucket, msg
                    )))))
                }
            }
        }
    }

    async fn set_public_policy(&self, bucket: &str) {
        let policy = format!(
            r#"{{"Version":"2012-10-17","Statement":[{{"Effect":"Allow","Principal":{{"AWS":["*"]}},"Action":["s3:GetObject"],"Resource":["arn:aws:s3:::{}/*"]}}]}}"#,
            bucket
        );
        match self.client.put_bucket_policy().bucket(bucket).policy(policy).send().await {
            Ok(_) => tracing::info!("Successfully set public bucket policy for {}", bucket),
            Err(e) => {
                tracing::warn!("Failed to set public bucket policy for {}: {}", bucket, e);
                tracing::warn!("Make sure objects in this bucket are publicly readable (configure it in your S3 provider), otherwise uploaded files will not be accessible.");
            }
        }
    }

    async fn set_backup_lifecycle(&self, bucket: &str) {
        use aws_sdk_s3::types::{ExpirationStatus, LifecycleRule, LifecycleRuleFilter};
        let rule = LifecycleRule::builder()
            .id("auto-delete-backups")
            .status(ExpirationStatus::Enabled)
            .filter(LifecycleRuleFilter::builder().prefix("").build())
            .expiration(
                aws_sdk_s3::types::LifecycleExpiration::builder()
                    .days(1)
                    .build(),
            )
            .build()
            .expect("valid lifecycle rule");
        let cfg = aws_sdk_s3::types::BucketLifecycleConfiguration::builder()
            .rules(rule)
            .build()
            .expect("valid lifecycle configuration");
        match self
            .client
            .put_bucket_lifecycle_configuration()
            .bucket(bucket)
            .lifecycle_configuration(cfg)
            .send()
            .await
        {
            Ok(_) => tracing::info!("Successfully set lifecycle policy for backups (auto-delete after 24h)"),
            Err(e) => tracing::warn!("Failed to set lifecycle policy for backups: {}", e),
        }
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
            if let Err(e) = self.client.delete_object().bucket(&self.bucket).key(&key).send().await {
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
        req.presigned(cfg).await.map(|p| p.uri().to_string()).map_err(|e| e.to_string())
    }

    /// Storage cleanup for a deleted resource node: metadata path first, then
    /// prefix sweep (Go DeleteSingleResource parity).
    pub async fn delete_resource_files(&self, user_id: i64, node_id: i64, metadata: Option<&serde_json::Value>) {
        if let Some(meta) = metadata
            && let Some(path) = meta.get("transformed_path").and_then(|v| v.as_str())
                && !path.is_empty() {
                    self.delete_user_object(user_id, path).await;
                    return;
                }
        self.delete_node_prefix(user_id, node_id).await;
    }
}
