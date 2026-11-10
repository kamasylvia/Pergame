//! S3 fan-out target: byte-level replica store on any S3-compatible endpoint
//! (RustFS/MinIO/Garage/AWS). Objects live under the configured prefix.

use aws_sdk_s3::Client;
use aws_sdk_s3::config::{Builder as S3ConfigBuilder, Credentials};
use aws_sdk_s3::primitives::ByteStream;

use super::{Fingerprint, ObjectRow, S3StoreParams, internal, join_prefix};

pub struct S3Target {
    client: Client,
    bucket: String,
    prefix: String,
}

impl S3Target {
    pub fn new(p: &S3StoreParams) -> Self {
        let url = if p.endpoint.contains("://") {
            p.endpoint.clone()
        } else {
            format!(
                "{}://{}",
                if p.secure { "https" } else { "http" },
                p.endpoint
            )
        };
        let cfg = S3ConfigBuilder::new()
            .behavior_version(aws_sdk_s3::config::BehaviorVersion::latest())
            .endpoint_url(url)
            .region(aws_sdk_s3::config::Region::new(p.region.clone()))
            .credentials_provider(Credentials::new(
                &p.access_key,
                &p.secret_key,
                None,
                None,
                "BackupTarget",
            ))
            .force_path_style(true)
            .build();
        Self {
            client: Client::from_conf(cfg),
            bucket: p.bucket.clone(),
            prefix: p.prefix.trim().trim_matches('/').to_string(),
        }
    }

    fn key(&self, path: &str) -> String {
        join_prefix(&self.prefix, path)
    }

    /// Physical identity probe: bucket must exist (HEAD). The fingerprint
    /// itself is computed from config (endpoint|bucket) by the caller.
    pub async fn probe(&self) -> Result<(), crate::error::ApiError> {
        self.client
            .head_bucket()
            .bucket(&self.bucket)
            .send()
            .await
            .map(|_| ())
            .map_err(|e| {
                super::internal(format!(
                    "s3 target probe failed for bucket {}: {e}",
                    self.bucket
                ))
            })
    }

    pub async fn has(&self, path: &str) -> Result<bool, crate::error::ApiError> {
        match self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(self.key(path))
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("404") || msg.contains("NoSuchKey") || msg.contains("NotFound") {
                    Ok(false)
                } else {
                    Err(internal(msg))
                }
            }
        }
    }

    pub async fn put(&self, path: &str, bytes: Vec<u8>) -> Result<(), crate::error::ApiError> {
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(self.key(path))
            .body(ByteStream::from(bytes))
            .send()
            .await
            .map(|_| ())
            .map_err(|e| internal(format!("s3 put {}: {e}", self.key(path))))
    }

    pub async fn get(&self, path: &str) -> Result<Vec<u8>, crate::error::ApiError> {
        let resp = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(self.key(path))
            .send()
            .await
            .map_err(|e| internal(format!("s3 get {}: {e}", self.key(path))))?;
        let bytes = resp.body.collect().await.map_err(internal)?;
        Ok(bytes.to_vec())
    }

    pub async fn delete(&self, path: &str) -> Result<bool, crate::error::ApiError> {
        let _res = self
            .client
            .delete_object()
            .bucket(&self.bucket)
            .key(self.key(path))
            .send()
            .await
            .map_err(|e| internal(format!("s3 delete {}: {e}", self.key(path))))?;
        Ok(true)
    }

    /// Lists objects under `path` prefix (recursive), oldest mtime first.
    pub async fn list(&self, path: &str) -> Result<Vec<ObjectRow>, crate::error::ApiError> {
        let prefix = self.key(path);
        let mut out = Vec::new();
        let mut token: Option<String> = None;
        loop {
            let mut req = self
                .client
                .list_objects_v2()
                .bucket(&self.bucket)
                .prefix(&prefix);
            if let Some(t) = token.clone() {
                req = req.continuation_token(t);
            }
            let resp = req
                .send()
                .await
                .map_err(|e| internal(format!("s3 list: {e}")))?;
            for obj in resp.contents() {
                let Some(key) = obj.key() else { continue };
                let size = obj.size().unwrap_or(0);
                let mtime = obj
                    .last_modified()
                    .and_then(|t| t.to_millis().ok())
                    .unwrap_or(0);
                out.push((key.to_string(), size, mtime));
            }
            token = resp.next_continuation_token().map(|t| t.to_string());
            if !resp.is_truncated().unwrap_or(false) || token.is_none() {
                break;
            }
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(out)
    }

    /// Blob referenced-hash set under a directory (for GC/refcount).
    pub async fn list_blob_keys(
        &self,
        user_id: i64,
    ) -> Result<Vec<String>, crate::error::ApiError> {
        let mut out = Vec::new();
        for dir in ["blobs", "blobs-e2e"] {
            let prefix = self.key(&format!("{user_id}/{dir}/"));
            for (key, _, _) in self.list(&prefix).await? {
                if let Some(sha) = key.rsplit('/').next() {
                    out.push(sha.to_string());
                }
            }
        }
        Ok(out)
    }
}

/// Convenience: fingerprint from config (endpoint|bucket, normalized).
pub fn fingerprint(p: &S3StoreParams) -> Fingerprint {
    Fingerprint(super::s3_identity(&p.endpoint, &p.bucket))
}
