//! /resources download routes — authed file/avatar/legacy bytes with read authorization.

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;
use serde::Deserialize;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::auth::guard::{self, Actor, NodeAction};
use crate::error::ApiError;
use crate::models::NodeRow;
use crate::repo::nodes as node_repo;

#[derive(Debug, Deserialize)]
/// GET file query (`download=1` forces attachment disposition).
pub struct DownloadQuery {
    #[serde(default)]
    pub download: bool,
}

/// S3 object location resolved from a resource node: either a content hash
/// (`content_sha256` → `blobs/{sha}`, deduped) or a legacy per-user key
/// (`transformed_path` under `{user_id}/`), plus MIME type and filename.
struct ObjectRef {
    key: String,
    mime: String,
    filename: String,
    sha: Option<String>,
}

/// Pass-through stream that logs S3 mid-body failures (otherwise axum just
/// aborts the connection and the client sees a truncated body).
struct LogStream<S> {
    inner: S,
    key: String,
}

impl<S> futures_core::Stream for LogStream<S>
where
    S: futures_core::Stream<Item = Result<bytes::Bytes, std::io::Error>> + Unpin,
{
    type Item = Result<bytes::Bytes, std::io::Error>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Result<bytes::Bytes, std::io::Error>>> {
        match std::pin::Pin::new(&mut self.inner).poll_next(cx) {
            std::task::Poll::Ready(Some(Err(e))) => {
                tracing::warn!(key = %self.key, error = %e, "s3 download stream failed mid-body");
                std::task::Poll::Ready(Some(Err(e)))
            }
            other => other,
        }
    }
}

fn object_ref(node: &NodeRow) -> Option<ObjectRef> {
    let meta = node.metadata.as_ref()?;
    let key = meta.get("transformed_path")?.as_str()?;
    if key.is_empty() {
        return None;
    }
    let sha = meta
        .get("content_sha256")
        .and_then(|v| v.as_str())
        .filter(|s| s.len() == 64)
        .map(|s| s.to_string());
    Some(ObjectRef {
        key: key.to_string(),
        mime: meta
            .get("filetype")
            .and_then(|v| v.as_str())
            .unwrap_or("application/octet-stream")
            .to_string(),
        filename: meta
            .get("original_path")
            .and_then(|v| v.as_str())
            .unwrap_or(&node.name)
            .to_string(),
        sha,
    })
}

/// Authorization for a resource read: owner/app-admin pass; other signed-in
/// users need node Read (ancestor-chain permission) or a public subtree;
/// anonymous callers need the node (or an ancestor) to be public
/// (`get_public`, accessibility = 3), which is what lets public-doc embeds
/// load without a session.
async fn authorize_read(
    state: &AppState,
    auth: Option<&AuthUser>,
    node_id: Uuid,
) -> Result<NodeRow, ApiError> {
    let node = node_repo::get_by_id(&state.db, node_id)
        .await
        .map_err(ApiError::from)?
        .ok_or(ApiError::NotFound("not found".into()))?;
    if let Some(a) = auth {
        let actor: Actor = a.clone().into();
        if guard::check_node_action(&state.db, &actor, &node, NodeAction::Read)
            .await
            .is_ok()
        {
            return Ok(node);
        }
        // Signed in but no grant: public subtrees stay readable (same rule
        // as anonymous callers).
    }
    node_repo::get_public(&state.db, node_id)
        .await
        .map_err(ApiError::from)?
        .ok_or(ApiError::Forbidden("forbidden".into()))
}

/// Length of a `bytes=a-b` range (None for suffix/open forms — those stream).
fn parse_range_len(range: &str) -> Option<u64> {
    let spec = range.strip_prefix("bytes=")?;
    let (a, b) = spec.split_once('-')?;
    let start: u64 = a.parse().ok()?;
    let end: u64 = b.parse().ok()?;
    end.checked_sub(start)?.checked_add(1)
}

/// Full-body variant of `serve_object` for small ranges: collects the S3
/// bytes before responding, so a mid-body stream abort becomes a retried
/// request instead of a truncated 206.
async fn serve_buffered(
    storage: &crate::storage::S3Storage,
    user_id: Uuid,
    obj: &ObjectRef,
    range: Option<&str>,
    download: bool,
    cache_sha: Option<&str>,
    state: &AppState,
) -> Result<Response, ApiError> {
    let object_key = match &obj.sha {
        Some(sha) => format!("{}/{}", crate::blobs::BLOBS_PREFIX, sha),
        None => format!("{user_id}/{}", obj.key),
    };
    let mut last_err = String::new();
    for attempt in 0..3 {
        let mut req = storage
            .client
            .get_object()
            .bucket(&storage.bucket)
            .key(&object_key);
        if let Some(r) = range {
            req = req.range(r);
        }
        match req.send().await {
            Ok(resp) => {
                let status = if resp.content_range.is_some() {
                    StatusCode::PARTIAL_CONTENT
                } else {
                    StatusCode::OK
                };
                match resp.body.collect().await {
                    Ok(data) => {
                        let bytes = data.into_bytes();
                        // Full bodies fill the disk cache; a partial range
                        // must never be stored as the whole blob.
                        if range.is_none()
                            && let (Some(sha), Some(cache)) = (cache_sha, state.disk_cache.as_ref())
                        {
                            let _ = cache.put(sha, &bytes).await;
                        }
                        let mut builder = Response::builder().status(status);
                        builder = builder.header(header::CONTENT_TYPE, obj.mime.clone());
                        builder = builder.header(header::ACCEPT_RANGES, "bytes");
                        builder = builder.header(header::CACHE_CONTROL, cache_control_for(obj));
                        builder = builder.header(header::CONTENT_LENGTH, bytes.len().to_string());
                        if let Some(cr) = resp.content_range {
                            builder = builder.header(header::CONTENT_RANGE, cr);
                        }
                        if let Some(tag) = etag_for(obj).or(resp.e_tag) {
                            builder = builder.header(header::ETAG, tag);
                        }
                        if download {
                            builder = builder.header(
                                header::CONTENT_DISPOSITION,
                                format!(
                                    "attachment; filename=\"{}\"",
                                    obj.filename.replace('"', "_")
                                ),
                            );
                        }
                        return builder.body(Body::from(bytes)).map_err(|e| {
                            ApiError::Internal(Box::new(std::io::Error::other(e.to_string())))
                        });
                    }
                    Err(e) => {
                        last_err = e.to_string();
                        tracing::warn!(%object_key, attempt, error = %e, "buffered range body failed, retrying");
                    }
                }
            }
            Err(e) => {
                last_err = e.to_string();
                tracing::warn!(%object_key, attempt, error = %e, "buffered range request failed, retrying");
            }
        }
    }
    Err(ApiError::BadRequest(format!(
        "object read failed: {last_err}"
    )))
}

/// Cache header for an attachment: content-addressed blobs never change, so
/// the browser may keep them forever; legacy keys are mutable-ish.
fn cache_control_for(obj: &ObjectRef) -> &'static str {
    if obj.sha.is_some() {
        "private, max-age=31536000, immutable"
    } else {
        "private, max-age=300"
    }
}

/// ETag for a blob: the sha is the content identity.
fn etag_for(obj: &ObjectRef) -> Option<String> {
    obj.sha.as_ref().map(|sha| format!("\"sha256-{sha}\""))
}

/// True when the client's `If-None-Match` already names this ETag.
fn if_none_match_hit(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(',')
                .map(str::trim)
                .any(|candidate| candidate == etag || candidate == "*")
        })
}

/// `bytes=a-b` / `a-` / `-n` resolved against the file length.
fn parse_range_bounds(range: &str, total: u64) -> Option<(u64, u64)> {
    let spec = range.strip_prefix("bytes=")?;
    let (a, b) = spec.split_once('-')?;
    if a.is_empty() {
        let suffix: u64 = b.parse().ok()?;
        if suffix == 0 {
            return None;
        }
        let start = total.saturating_sub(suffix);
        return Some((start, total));
    }
    let start: u64 = a.parse().ok()?;
    if start >= total {
        return None;
    }
    let end = if b.is_empty() {
        total
    } else {
        b.parse::<u64>().ok()?.min(total.saturating_sub(1)) + 1
    };
    Some((start, end.max(start + 1)))
}

/// Serve one blob from the local disk cache (Range supported via seek).
async fn serve_disk_file(
    path: &std::path::Path,
    obj: &ObjectRef,
    headers: &HeaderMap,
    download: bool,
) -> Result<Response, ApiError> {
    use tokio::io::{AsyncReadExt, AsyncSeekExt};

    let mut file = tokio::fs::File::open(path).await.map_err(|e| {
        ApiError::Internal(Box::new(std::io::Error::other(format!(
            "cache read failed: {e}"
        ))))
    })?;
    let total = file
        .metadata()
        .await
        .map_err(|e| ApiError::Internal(Box::new(std::io::Error::other(e.to_string()))))?
        .len();
    let range = headers
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .filter(|v| v.starts_with("bytes="));

    let mut builder = Response::builder();
    builder = builder.header(header::CONTENT_TYPE, obj.mime.clone());
    builder = builder.header(header::ACCEPT_RANGES, "bytes");
    builder = builder.header(header::CACHE_CONTROL, cache_control_for(obj));
    if let Some(etag) = etag_for(obj) {
        builder = builder.header(header::ETAG, etag);
    }
    if download {
        builder = builder.header(
            header::CONTENT_DISPOSITION,
            format!(
                "attachment; filename=\"{}\"",
                obj.filename.replace('"', "_")
            ),
        );
    }

    match range.and_then(|r| parse_range_bounds(r, total)) {
        Some((start, end)) => {
            file.seek(std::io::SeekFrom::Start(start))
                .await
                .map_err(|e| ApiError::Internal(Box::new(std::io::Error::other(e.to_string()))))?;
            let limited = file.take(end - start);
            let reader = tokio_util::io::ReaderStream::new(limited);
            builder = builder.status(StatusCode::PARTIAL_CONTENT);
            builder = builder.header(header::CONTENT_LENGTH, (end - start).to_string());
            builder = builder.header(
                header::CONTENT_RANGE,
                format!("bytes {}-{}/{}", start, end - 1, total),
            );
            builder
                .body(Body::from_stream(reader))
                .map_err(|e| ApiError::Internal(Box::new(std::io::Error::other(e.to_string()))))
        }
        None => {
            let reader = tokio_util::io::ReaderStream::new(file);
            builder = builder.header(header::CONTENT_LENGTH, total.to_string());
            builder
                .body(Body::from_stream(reader))
                .map_err(|e| ApiError::Internal(Box::new(std::io::Error::other(e.to_string()))))
        }
    }
}

/// Builds the file response: disk cache first (immutable blobs), then S3.
///
/// Small responses are buffered, not streamed, because the S3 byte stream
/// intermittently aborts mid-body on the first request of a cold connection
/// ("streaming error", QuObjects over Tailscale), truncating the response.
/// Ranges ≤8MB and full bodies ≤32MB are cheap to buffer and immune (with
/// retries); larger bodies keep streaming. A buffered body is written to the
/// disk cache, so the next viewer never touches S3.
async fn serve_object(
    state: &AppState,
    user_id: Uuid,
    obj: &ObjectRef,
    headers: &HeaderMap,
    download: bool,
) -> Result<Response, ApiError> {
    // Content-addressed blobs are immutable: an ETag match is a free 304.
    if let Some(etag) = etag_for(obj)
        && if_none_match_hit(headers, &etag)
    {
        let mut builder = Response::builder().status(StatusCode::NOT_MODIFIED);
        builder = builder.header(header::ETAG, etag);
        builder = builder.header(header::CACHE_CONTROL, cache_control_for(obj));
        return builder
            .body(Body::empty())
            .map_err(|e| ApiError::Internal(Box::new(std::io::Error::other(e.to_string()))));
    }

    if let (Some(sha), Some(cache)) = (obj.sha.as_deref(), state.disk_cache.as_ref()) {
        let flight = state.download_flights.lock(sha).await;
        let cached = cache.get(sha).await;
        if let Some(path) = cached {
            drop(flight);
            state.download_flights.release(sha).await;
            return serve_disk_file(&path, obj, headers, download).await;
        }
        let result = serve_blob_via_s3(state, user_id, obj, headers, download, Some(sha)).await;
        drop(flight);
        state.download_flights.release(sha).await;
        return result;
    }

    serve_blob_via_s3(state, user_id, obj, headers, download, None).await
}

/// Fetch from S3 (buffered when small), optionally filling the disk cache.
async fn serve_blob_via_s3(
    state: &AppState,
    user_id: Uuid,
    obj: &ObjectRef,
    headers: &HeaderMap,
    download: bool,
    cache_sha: Option<&str>,
) -> Result<Response, ApiError> {
    let storage = state
        .storage
        .clone()
        .ok_or(ApiError::BadRequest("minio client not initialized".into()))?;
    const BUFFERED_RANGE_CAP: u64 = 8 * 1024 * 1024;
    const BUFFERED_FULL_CAP: i64 = 32 * 1024 * 1024;
    let range = headers
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .filter(|v| v.starts_with("bytes="));
    if let Some(range_len) = range.and_then(parse_range_len)
        && range_len <= BUFFERED_RANGE_CAP
    {
        return serve_buffered(&storage, user_id, obj, range, download, cache_sha, state).await;
    }
    if range.is_none() {
        let object_key = match &obj.sha {
            Some(sha) => format!("{}/{}", crate::blobs::BLOBS_PREFIX, sha),
            None => format!("{user_id}/{}", obj.key),
        };
        if let Some(len) = storage.object_len(&object_key).await
            && len <= BUFFERED_FULL_CAP
        {
            return serve_buffered(&storage, user_id, obj, None, download, cache_sha, state).await;
        }
    }
    let (stream, len, content_range, etag) = match &obj.sha {
        Some(sha) => storage.get_blob_stream(sha, range).await?,
        None => {
            storage
                .get_user_object_stream(user_id, &obj.key, range)
                .await?
        }
    };
    let key = match &obj.sha {
        Some(sha) => format!("{}/{}", crate::blobs::BLOBS_PREFIX, sha),
        None => format!("{user_id}/{}", obj.key),
    };
    let reader = tokio_util::io::ReaderStream::new(stream.into_async_read());
    let logged = LogStream { inner: reader, key };
    let body = Body::from_stream(logged);
    let mut builder = Response::builder();
    if content_range.is_some() {
        builder = builder.status(StatusCode::PARTIAL_CONTENT);
    }
    builder = builder.header(header::CONTENT_TYPE, obj.mime.clone());
    builder = builder.header(header::ACCEPT_RANGES, "bytes");
    builder = builder.header(header::CACHE_CONTROL, cache_control_for(obj));
    if let Some(n) = len {
        builder = builder.header(header::CONTENT_LENGTH, n.to_string());
    }
    if let Some(cr) = content_range {
        builder = builder.header(header::CONTENT_RANGE, cr);
    }
    if let Some(tag) = etag_for(obj).or(etag) {
        builder = builder.header(header::ETAG, tag);
    }
    if download {
        builder = builder.header(
            header::CONTENT_DISPOSITION,
            format!(
                "attachment; filename=\"{}\"",
                obj.filename.replace('"', "_")
            ),
        );
    }
    builder
        .body(body)
        .map_err(|e| ApiError::Internal(Box::new(std::io::Error::other(e.to_string()))))
}

/// GET /resources/:id/file — authed file download (fork addition, no Go
/// counterpart: upstream served bytes straight off the public bucket).
/// Anonymous access is allowed only for public-subtree nodes.
pub async fn download_file(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
    Path(id): Path<String>,
    Query(q): Query<DownloadQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let node_id =
        Uuid::parse_str(&id).map_err(|_| ApiError::BadRequest("invalid parameter".into()))?;
    let node = authorize_read(&state, auth.as_ref(), node_id).await?;
    if node.role != 4 {
        return Err(ApiError::BadRequest("not a resource".into()));
    }
    let obj = object_ref(&node).ok_or(ApiError::NotFound("object not found".into()))?;
    serve_object(&state, node.user_id, &obj, &headers, q.download).await
}

/// GET /resources/:id/file/:name — same bytes, with the original filename in
/// the path. Markdown renderers (and browsers saving a file) decide by the
/// extension: `/file` alone cannot tell a PDF from an image, so an embedded
/// PDF would render as a plain link.
pub async fn download_file_named(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
    Path((id, _name)): Path<(String, String)>,
    Query(q): Query<DownloadQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    download_file(State(state), auth, Path(id), Query(q), headers).await
}

/// GET /users/:id/avatar — public avatar bytes (no node exists for avatars;
/// the users row decides: missing/empty `avatar` → 404, frontend falls back
/// to the placeholder). Unauthenticated by design: author avatars render on
/// public docs.
pub async fn download_avatar(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let uid = Uuid::parse_str(&id).map_err(|_| ApiError::BadRequest("Invalid user ID".into()))?;
    let mut db = state.toasty.clone();
    let has_avatar = crate::models::User::filter_by_id(uid)
        .first()
        .exec(&mut db)
        .await
        .ok()
        .flatten()
        .and_then(|u| u.avatar)
        .is_some_and(|a| !a.is_empty());
    if !has_avatar {
        return Err(ApiError::NotFound("not found".into()));
    }
    let obj = ObjectRef {
        key: "avatar".to_string(),
        mime: "image/*".to_string(),
        filename: "avatar".to_string(),
        sha: None,
    };
    serve_object(&state, uid, &obj, &headers, false).await
}

/// Legacy CDN path compat: old `content_compiled` embeds point at
/// `/pergame/{user_id}/{transformed_path}`. Resolves the object back to its
/// node and applies the same authorization as `download_file`.
pub async fn download_legacy(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
    Path((user_id, file)): Path<(String, String)>,
    Query(q): Query<DownloadQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let uid =
        Uuid::parse_str(&user_id).map_err(|_| ApiError::BadRequest("invalid parameter".into()))?;
    let node_id = file.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(&file);
    // Avatar legacy path has no node; same public rule as download_avatar.
    if node_id == "avatar" {
        return download_avatar(State(state), Path(user_id), headers).await;
    }
    let node_id =
        Uuid::parse_str(node_id).map_err(|_| ApiError::BadRequest("invalid parameter".into()))?;
    let node = authorize_read(&state, auth.as_ref(), node_id).await?;
    if node.role != 4 || node.user_id != uid {
        return Err(ApiError::NotFound("not found".into()));
    }
    let obj = object_ref(&node).ok_or(ApiError::NotFound("object not found".into()))?;
    if obj.key != file {
        return Err(ApiError::NotFound("not found".into()));
    }
    serve_object(&state, node.user_id, &obj, &headers, q.download).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(meta: Option<serde_json::Value>) -> NodeRow {
        NodeRow {
            id: uuid::Uuid::nil(),
            user_id: uuid::Uuid::nil(),
            parent_id: None,
            name: "fallback.txt".into(),
            description: None,
            tags: None,
            role: 4,
            color: None,
            icon: None,
            thumbnail: None,
            theme: None,
            accessibility: None,
            access: 0,
            display: None,
            node_order: Some(0),
            content: None,
            content_compiled: None,
            size: None,
            metadata: meta,
            created_timestamp: 0,
            updated_timestamp: 0,
        }
    }

    #[test]
    fn object_ref_needs_transformed_path() {
        assert!(object_ref(&node(None)).is_none());
        assert!(object_ref(&node(Some(serde_json::json!({"filetype": "text/plain"})))).is_none());
        let meta = serde_json::json!({
            "transformed_path": "u/abc.txt",
            "filetype": "text/plain",
            "original_path": "abc.txt",
        });
        let obj = object_ref(&node(Some(meta))).unwrap();
        assert_eq!(obj.key, "u/abc.txt");
        assert_eq!(obj.mime, "text/plain");
        assert_eq!(obj.filename, "abc.txt");
        // Missing mime/original fall back to octet-stream and node name.
        let bare = object_ref(&node(Some(serde_json::json!({"transformed_path": "u/x"})))).unwrap();
        assert_eq!(bare.mime, "application/octet-stream");
        assert_eq!(bare.filename, "fallback.txt");
    }
}
