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
//! env var logs a warning when set). Endpoint scheme normalization and the
//! localhost→HTTP fallback mirror the Go behavior.
//!
//! Single bucket: only `S3_BUCKET` is created. Snapshots write straight to
//! the user's own backup targets (2026-11-12 direct-write); the legacy
//! `{bucket}-backups` service bucket is gone.

use aws_sdk_s3::config::{Builder as S3ConfigBuilder, Credentials};
use aws_smithy_http_client::tls::{TlsContext, TrustStore};
use aws_smithy_runtime_api::client::http::SharedHttpClient;

use aws_sdk_s3::Client;
use aws_sdk_s3::operation::create_bucket::CreateBucketOutput;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiError;

#[derive(Debug, Clone)]
/// Service-bucket client (attachments + inline zips; snapshots live on targets).
pub struct S3Storage {
    pub client: Client,
    pub bucket: String,
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

/// Build from `S3_ENDPOINT/S3_ACCESSKEY/S3_SECRETKEY` (+ optional `S3_BUCKET`).
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
        .force_path_style(true)
        // Slow S3 links (QuObjects over Tailscale, ~33KB/s) stall longer
        // than the SDK's 20s default grace, aborting big downloads with
        // "streaming error". Disable download-side protection; uploads
        // keep it (fast local reads never legitimately stall).
        .stalled_stream_protection(
            aws_sdk_s3::config::StalledStreamProtectionConfig::enabled()
                .download_enabled(false)
                .build(),
        );

    if let Some(ca_path) = env("S3_CA_PATH") {
        let http_client = load_custom_ca_client(&ca_path)?;
        s3cfg = s3cfg.http_client(http_client);
    }

    let s3cfg = s3cfg.build();

    Ok(S3Storage {
        client: Client::from_conf(s3cfg),
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
    /// Creates the main bucket idempotently. No bucket policy is set: all
    /// reads go through the backend's authed routes, so the bucket stays
    /// private (2026-11-12 CDN cutover; providers without PutBucketPolicy
    /// like QuObjects warn-free now).
    pub async fn ensure_buckets(&self) -> Result<(), ApiError> {
        self.ensure_bucket(&self.bucket).await?;
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

    /// PUT bytes at the content-addressed key `blobs/{sha256}`.
    pub async fn put_blob(&self, sha: &str, content: Vec<u8>) -> Result<(), ApiError> {
        let object_key = format!("{}/{}", crate::blobs::BLOBS_PREFIX, sha);
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&object_key)
            .body(content.into())
            .send()
            .await
            .map(|_| ())
            .map_err(|e| {
                tracing::error!(error = %e, %object_key, "put_blob failed");
                ApiError::Internal(Box::new(std::io::Error::other("failed to store file")))
            })
    }

    /// GET bytes from `blobs/{sha256}`.
    pub async fn get_blob(&self, sha: &str) -> Result<Vec<u8>, ApiError> {
        let object_key = format!("{}/{}", crate::blobs::BLOBS_PREFIX, sha);
        let resp = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&object_key)
            .send()
            .await
            .map_err(|e| {
                tracing::warn!(%object_key, error = %e, "get_blob failed");
                ApiError::BadRequest("object not found".into())
            })?;
        let bytes = resp
            .body
            .collect()
            .await
            .map_err(|e| {
                tracing::error!(%object_key, error = %e, "blob body read failed");
                ApiError::BadRequest("failed to read object".into())
            })?
            .into_bytes();
        Ok(bytes.to_vec())
    }

    /// Streaming GET from `blobs/{sha256}` with optional Range. Mirrors
    /// `get_user_object_stream` return shape (body, len, range, etag).
    pub async fn get_blob_stream(
        &self,
        sha: &str,
        range: Option<&str>,
    ) -> Result<
        (
            aws_sdk_s3::primitives::ByteStream,
            Option<i64>,
            Option<String>,
            Option<String>,
        ),
        ApiError,
    > {
        let object_key = format!("{}/{}", crate::blobs::BLOBS_PREFIX, sha);
        let mut req = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&object_key);
        if let Some(r) = range {
            req = req.range(r);
        }
        let resp = req.send().await.map_err(|e| {
            tracing::warn!(%object_key, error = %e, "get_blob failed");
            ApiError::NotFound("object not found".into())
        })?;
        Ok((
            resp.body,
            resp.content_length,
            resp.content_range,
            resp.e_tag,
        ))
    }

    /// Delete `blobs/{sha256}` (best-effort; refcount already hit 0).
    pub async fn delete_blob(&self, sha: &str) {
        let object_key = format!("{}/{}", crate::blobs::BLOBS_PREFIX, sha);
        if let Err(e) = self
            .client
            .delete_object()
            .bucket(&self.bucket)
            .key(&object_key)
            .send()
            .await
        {
            tracing::warn!(%object_key, error = %e, "failed to delete blob");
        }
    }

    /// Byte length of a blob or user object (HEAD, no body). Used to decide
    /// buffered vs streamed downloads. `None` on any failure (caller streams).
    pub async fn object_len(&self, object_key: &str) -> Option<i64> {
        self.client
            .head_object()
            .bucket(&self.bucket)
            .key(object_key)
            .send()
            .await
            .ok()
            .and_then(|o| o.content_length)
    }

    /// PUT object under `{user_id}/{key}`.
    pub async fn put_user_object(
        &self,
        user_id: Uuid,
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

    /// Streaming GET under `{user_id}/{key}` with optional S3 range
    /// (`bytes=a-b`, suffix and open-ended forms). Returns
    /// (body, content_length, content_range, etag): `content_range` is
    /// `Some` only when S3 honored the range (206).
    pub async fn get_user_object_stream(
        &self,
        user_id: Uuid,
        key: &str,
        range: Option<&str>,
    ) -> Result<
        (
            aws_sdk_s3::primitives::ByteStream,
            Option<i64>,
            Option<String>,
            Option<String>,
        ),
        ApiError,
    > {
        let object_key = format!("{}/{}", user_id, key);
        let mut req = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&object_key);
        if let Some(r) = range {
            req = req.range(r);
        }
        let resp = req.send().await.map_err(|e| {
            tracing::warn!(%object_key, error = %e, "get_object failed");
            ApiError::NotFound("object not found".into())
        })?;
        Ok((
            resp.body,
            resp.content_length,
            resp.content_range,
            resp.e_tag,
        ))
    }
    /// Lists keys under `{user_id}/{sub}/` in the main bucket (key, size,
    /// mtime_ms). Used by the inline-zip sweeper.
    pub async fn list_user_prefix(
        &self,
        user_id: Uuid,
        sub: &str,
    ) -> Result<Vec<(String, i64, i64)>, ApiError> {
        let prefix = format!("{user_id}/{sub}/");
        let mut out = Vec::new();
        let resp = self
            .client
            .list_objects_v2()
            .bucket(&self.bucket)
            .prefix(&prefix)
            .send()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "list user prefix failed");
                ApiError::Internal(Box::new(std::io::Error::other("failed to list objects")))
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

    /// Direct children of `{user_id}/` — the user's resource objects (uploads
    /// and the avatar). Nested prefixes such as `inline-zips/` are excluded:
    /// those are managed by their own sweepers, not by resource cleanup.
    pub async fn list_user_objects(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<(String, i64, i64)>, ApiError> {
        let prefix = format!("{user_id}/");
        let resp = self
            .client
            .list_objects_v2()
            .bucket(&self.bucket)
            .prefix(&prefix)
            .send()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "list user objects failed");
                ApiError::Internal(Box::new(std::io::Error::other("failed to list objects")))
            })?;
        let mut out = Vec::new();
        for obj in resp.contents() {
            let Some(key) = obj.key() else { continue };
            let Some(rest) = key.strip_prefix(&prefix) else {
                continue;
            };
            if rest.is_empty() || rest.contains('/') {
                continue;
            }
            let size = obj.size().unwrap_or(0);
            let mtime = obj
                .last_modified()
                .and_then(|t| t.to_millis().ok())
                .unwrap_or(0);
            out.push((rest.to_string(), size, mtime));
        }
        Ok(out)
    }

    /// Deletes one direct child of `{user_id}/`, surfacing failures (the
    /// best-effort `delete_user_object` is for cascades that must not abort).
    pub async fn delete_user_object_checked(
        &self,
        user_id: Uuid,
        key: &str,
    ) -> Result<(), ApiError> {
        let object_key = format!("{user_id}/{key}");
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(&object_key)
            .send()
            .await
            .map(|_| ())
            .map_err(|e| {
                tracing::warn!(%object_key, error = %e, "delete object failed");
                ApiError::Internal(Box::new(std::io::Error::other("failed to delete object")))
            })
    }

    /// Deletes `{user_id}/{key}` (best-effort, Go parity: failures logged).
    pub async fn delete_user_object(&self, user_id: Uuid, key: &str) {
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

    /// PUT an object at an absolute key (plugin artifacts live under
    /// `plugins/<id>/<version>/…`, outside any user prefix).
    pub async fn put_object(
        &self,
        object_key: &str,
        content: Vec<u8>,
        content_type: Option<&str>,
    ) -> Result<(), ApiError> {
        let mut req = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(object_key)
            .body(content.into());
        if let Some(ct) = content_type {
            req = req.content_type(ct);
        }
        req.send().await.map(|_| ()).map_err(|e| {
            tracing::error!(error = %e, object_key, "put_object failed");
            ApiError::Internal(Box::new(std::io::Error::other("failed to store file")))
        })
    }

    /// GET an absolute key as bytes (plugin entry/manifest serving).
    pub async fn get_object_bytes(&self, object_key: &str) -> Result<Vec<u8>, ApiError> {
        let resp = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(object_key)
            .send()
            .await
            .map_err(|e| {
                tracing::warn!(object_key, error = %e, "get_object failed");
                ApiError::NotFound("object not found".into())
            })?;
        let body = resp.body.collect().await.map_err(|e| {
            tracing::warn!(object_key, error = %e, "get_object body failed");
            ApiError::Internal(Box::new(std::io::Error::other("failed to read object")))
        })?;
        Ok(body.into_bytes().to_vec())
    }

    /// Best-effort delete of everything under an absolute prefix (plugin
    /// removal). Failures are logged: the row is gone either way.
    pub async fn delete_prefix(&self, prefix: &str) {
        let Ok(list) = self
            .client
            .list_objects_v2()
            .bucket(&self.bucket)
            .prefix(prefix)
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
                tracing::warn!(%key, error = %e, "failed to delete plugin object");
            }
        }
    }

    /// Deletes everything under the `{user_id}/{node_id}` prefix (fallback
    /// sweep of a resource node, Go DeleteSingleResource fallback path).
    pub async fn delete_node_prefix(&self, user_id: Uuid, node_id: Uuid) {
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

    /// Deletes everything under the `{user_id}/` prefix (account deletion:
    /// attachments go away with the user; snapshots live on the user's own
    /// targets and are gated separately, never touched here).
    pub async fn delete_user_prefix(&self, user_id: Uuid) {
        let prefix = format!("{user_id}/");
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
    pub async fn get_user_object(&self, user_id: Uuid, key: &str) -> Result<Vec<u8>, ApiError> {
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

    /// Storage cleanup for a deleted resource node: deduped blobs release
    /// one reference (last one deletes the object); legacy per-user paths
    /// delete directly; unknown shapes fall back to the prefix sweep
    /// (Go DeleteSingleResource parity).
    pub async fn delete_resource_files(
        &self,
        pool: &PgPool,
        user_id: Uuid,
        node_id: Uuid,
        metadata: Option<&serde_json::Value>,
    ) {
        if let Some(meta) = metadata
            && let Some(sha) = meta.get("content_sha256").and_then(|v| v.as_str())
            && sha.len() == 64
        {
            crate::blobs::release(pool, self, sha).await;
            return;
        }
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
#[allow(unsafe_code)] // Test-only env mutation; no other test touches S3_*.
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

    #[test]
    fn parse_env_requires_trio_and_defaults_bucket() {
        // S3_* is only read by storage; no other test touches it.
        unsafe {
            std::env::remove_var("S3_ENDPOINT");
            std::env::remove_var("S3_ACCESSKEY");
            std::env::remove_var("S3_SECRETKEY");
            std::env::remove_var("S3_BUCKET");
        }
        assert!(parse_env().is_err());
        unsafe {
            std::env::set_var("S3_ENDPOINT", "localhost:9000");
            std::env::set_var("S3_ACCESSKEY", "ak");
            std::env::set_var("S3_SECRETKEY", "sk");
        }
        let s = parse_env().unwrap();
        assert_eq!(s.bucket, "pergame");
        unsafe {
            std::env::set_var("S3_BUCKET", "custom");
        }
        assert_eq!(parse_env().unwrap().bucket, "custom");
        unsafe {
            std::env::remove_var("S3_ENDPOINT");
            std::env::remove_var("S3_ACCESSKEY");
            std::env::remove_var("S3_SECRETKEY");
            std::env::remove_var("S3_BUCKET");
        }
    }
}
