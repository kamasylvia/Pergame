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
                        let mut builder = Response::builder().status(status);
                        builder = builder.header(header::CONTENT_TYPE, obj.mime.clone());
                        builder = builder.header(header::ACCEPT_RANGES, "bytes");
                        builder = builder.header(header::CONTENT_LENGTH, bytes.len().to_string());
                        if let Some(cr) = resp.content_range {
                            builder = builder.header(header::CONTENT_RANGE, cr);
                        }
                        if let Some(tag) = resp.e_tag {
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

/// Builds the file response: streams S3 bytes (Range passthrough for
/// audio/video seeks), sets content-type/length, and forces a download
/// filename only when `?download=true`.
async fn serve_object(
    state: &AppState,
    user_id: Uuid,
    obj: &ObjectRef,
    headers: &HeaderMap,
    download: bool,
) -> Result<Response, ApiError> {
    let storage = state
        .storage
        .clone()
        .ok_or(ApiError::BadRequest("minio client not initialized".into()))?;
    // Small ranges (pdf.js page fetches, media seeks) are buffered, not
    // streamed: the S3 byte stream intermittently aborts mid-body on the
    // first request of a cold connection ("streaming error", QuObjects over
    // Tailscale), truncating the response. Ranges ≤8MB are cheap to buffer
    // and immune; larger ranges and full bodies keep streaming.
    const BUFFERED_RANGE_CAP: u64 = 8 * 1024 * 1024;
    let range = headers
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .filter(|v| v.starts_with("bytes="));
    if let Some(range_len) = range.and_then(parse_range_len)
        && range_len <= BUFFERED_RANGE_CAP
    {
        return serve_buffered(&storage, user_id, obj, range, download).await;
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
    let logged = LogStream {
        inner: reader,
        key: key.clone(),
    };
    let body = Body::from_stream(logged);
    let mut builder = Response::builder();
    if content_range.is_some() {
        builder = builder.status(StatusCode::PARTIAL_CONTENT);
    }
    builder = builder.header(header::CONTENT_TYPE, obj.mime.clone());
    builder = builder.header(header::ACCEPT_RANGES, "bytes");
    if let Some(n) = len {
        builder = builder.header(header::CONTENT_LENGTH, n.to_string());
    }
    if let Some(cr) = content_range {
        builder = builder.header(header::CONTENT_RANGE, cr);
    }
    if let Some(tag) = etag {
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
