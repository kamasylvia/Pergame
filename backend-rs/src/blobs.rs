//! Content-addressed blob store: cross-user dedup for resource files.
//!
//! New uploads land at `blobs/{sha256}` (one copy per distinct bytes) instead
//! of `{user_id}/{uuid}.{ext}`. `object_blobs` tracks live references;
//! releasing the last reference deletes the S3 object. Legacy objects keep
//! their old keys and stay readable (nodes without `content_sha256` in
//! metadata); the admin migration (`POST /api/admin/storage/migrate-blobs`)
//! moves them over.
//!
//! Quotas intentionally count logical size (`nodes.size`), not physical
//! blobs: storing 17MB counts 17MB even when the bytes are shared.

use sha2::{Digest, Sha256};
use sqlx::PgPool;

use crate::error::ApiError;
use crate::storage::S3Storage;

/// Blob key prefix inside the service bucket.
pub const BLOBS_PREFIX: &str = "blobs";

/// SHA-256 hex of raw bytes (blob identity).
pub fn sha_of(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}
/// Store bytes if absent, bump the reference count, return the sha.
/// Idempotent under concurrency (INSERT .. ON CONFLICT DO UPDATE).
pub async fn acquire(pool: &PgPool, storage: &S3Storage, bytes: &[u8]) -> Result<String, ApiError> {
    let sha = sha_of(bytes);
    let size = bytes.len() as i64;
    let row: Option<(i32,)> = sqlx::query_as(
        "INSERT INTO object_blobs (sha256, size, refcount, created_timestamp)
         VALUES ($1, $2, 1, $3)
         ON CONFLICT (sha256) DO UPDATE SET refcount = object_blobs.refcount + 1
         RETURNING object_blobs.refcount",
    )
    .bind(&sha)
    .bind(size)
    .bind(crate::auth::service::now_ms())
    .fetch_optional(pool)
    .await
    .map_err(ApiError::from)?;
    // Upload only when we created the row: a concurrent uploader that won
    // the race already wrote identical bytes (same key, same content), and
    // S3 PUT is idempotent anyway.
    if row.map(|r| r.0) == Some(1)
        && let Err(e) = storage.put_blob(&sha, bytes.to_vec()).await
    {
        // Roll back the reservation so a retry uploads again.
        let _ = sqlx::query("DELETE FROM object_blobs WHERE sha256 = $1 AND refcount = 1")
            .bind(&sha)
            .execute(pool)
            .await;
        return Err(ApiError::Internal(Box::new(std::io::Error::other(
            e.to_string(),
        ))));
    }
    Ok(sha)
}

/// Release one reference; deletes the S3 object when the last one goes.
/// Missing rows are a no-op (already migrated away or never registered).
pub async fn release(pool: &PgPool, storage: &S3Storage, sha: &str) {
    let left: Option<(i32,)> = sqlx::query_as(
        "UPDATE object_blobs SET refcount = refcount - 1 WHERE sha256 = $1
         RETURNING refcount",
    )
    .bind(sha)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();
    match left {
        Some((n,)) if n <= 0 => {
            let _ = sqlx::query("DELETE FROM object_blobs WHERE sha256 = $1")
                .bind(sha)
                .execute(pool)
                .await;
            storage.delete_blob(sha).await;
        }
        _ => {}
    }
}

/// Current reference count (0 when the row is absent). Test/support helper.
pub async fn refcount(pool: &PgPool, sha: &str) -> i64 {
    sqlx::query_scalar::<_, i32>("SELECT refcount FROM object_blobs WHERE sha256 = $1")
        .bind(sha)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .map(i64::from)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha_is_stable_64_hex() {
        assert_eq!(sha_of(b"abc").len(), 64);
        assert_eq!(sha_of(b"abc"), sha_of(b"abc"));
        assert_ne!(sha_of(b"abc"), sha_of(b"abd"));
    }
}
