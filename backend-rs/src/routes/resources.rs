//! /resources routes — file upload, avatar upload, file update, authed file
//! download (fork: replaces the public-bucket CDN reads; see `download_file`).

use axum::body::Body;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::auth::guard::{self, Actor, NodeAction};
use crate::error::ApiError;
use crate::models::NodeRow;
use crate::repo::nodes as node_repo;
use crate::response::ApiOk;
const DEFAULT_TYPES: &[&str] = &[
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/bmp",
    "image/webp",
    "image/avif",
    "image/svg+xml",
    "image/vnd.microsoft.icon",
    "video/mp4",
    "video/x-msvideo",
    "video/quicktime",
    "video/x-ms-wmv",
    "video/x-flv",
    "video/x-matroska",
    "video/webm",
    "audio/mpeg",
    "audio/wav",
    "audio/ogg",
    "audio/flac",
    "audio/aac",
    "audio/webm",
    "application/pdf",
    "application/msword",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/vnd.ms-excel",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "application/vnd.ms-powerpoint",
    "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    "text/plain",
    "text/csv",
    "application/zip",
    "application/x-rar-compressed",
    "application/x-tar",
    "application/gzip",
    "application/x-bzip2",
];

const IMAGE_TYPES: &[&str] = &[
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/bmp",
    "image/webp",
    "image/avif",
    "image/svg+xml",
    "image/vnd.microsoft.icon",
];

fn supported_types() -> Vec<String> {
    let mut v: Vec<String> = crate::config::global()
        .cdn
        .supported_types
        .clone()
        .unwrap_or_default()
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect();
    if v.is_empty() {
        v = DEFAULT_TYPES.iter().map(|s| s.to_string()).collect();
    }
    v
}

fn supported_images() -> Vec<String> {
    let mut v: Vec<String> = crate::config::global()
        .cdn
        .supported_types_images
        .clone()
        .unwrap_or_default()
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect();
    if v.is_empty() {
        v = IMAGE_TYPES.iter().map(|s| s.to_string()).collect();
    }
    v
}

pub struct UploadedPart {
    pub filename: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

/// Pulls a single "file" part plus string fields from the multipart body.
async fn read_upload(
    mut multipart: Multipart,
) -> Result<
    (
        Option<UploadedPart>,
        std::collections::HashMap<String, String>,
    ),
    ApiError,
> {
    let mut file = None;
    let mut fields = std::collections::HashMap::new();
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        tracing::warn!(error = %e, "multipart next_field failed");
        ApiError::BadRequest("file too large or failed to get file".into())
    })? {
        let name = field.name().unwrap_or("").to_string();
        tracing::info!(%name, "multipart field seen");
        match name.as_str() {
            "file" => {
                let filename = field.file_name().unwrap_or("").to_string();
                let content_type = field.content_type().unwrap_or("").to_string();
                let bytes = field.bytes().await.map_err(|e| {
                    tracing::warn!(error = %e, "field bytes read failed");
                    ApiError::BadRequest(format!("file too large or failed to get file: {e}"))
                })?;
                file = Some(UploadedPart {
                    filename,
                    content_type,
                    bytes: bytes.to_vec(),
                });
            }
            name => {
                let value = field
                    .text()
                    .await
                    .map_err(|_| ApiError::BadRequest("failed to read form field".into()))?;
                fields.insert(name.to_string(), value);
            }
        }
    }
    Ok((file, fields))
}

fn cdn_limits() -> (u64, u64) {
    let cfg = crate::config::global();
    (cfg.cdn.max_size, cfg.cdn.max_uploads_size)
}

fn mime_allowed(list: &[String], mime: &str) -> bool {
    list.iter().any(|t| t == mime)
}

fn build_metadata(
    filetype: &str,
    original_path: &str,
    transformed_path: &str,
    extra: Option<&serde_json::Value>,
) -> serde_json::Value {
    let mut base = json!({
        "filetype": filetype,
        "original_path": original_path,
        "transformed_path": transformed_path,
    });
    // Go maps.Copy(metadata_final, metadata): user-provided keys override.
    if let (Some(base_obj), Some(extra_obj)) =
        (base.as_object_mut(), extra.and_then(|v| v.as_object()))
    {
        for (k, v) in extra_obj {
            base_obj.insert(k.clone(), v.clone());
        }
    }
    base
}

/// POST /resources — upload a file bound to an optional parent node.
pub async fn upload_file(
    State(state): State<AppState>,
    auth: AuthUser,
    multipart: Multipart,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let (max_file, max_total) = cdn_limits();
    let storage = state
        .storage
        .clone()
        .ok_or(ApiError::BadRequest("minio client not initialized".into()))?;

    let (file, fields) = read_upload(multipart).await?;
    let Some(file) = file else {
        tracing::warn!("debug: file field missing after read");
        return Err(ApiError::BadRequest(
            "file too large or failed to get file".into(),
        ));
    };

    let parent_id: Option<Uuid> = fields
        .get("parent_id")
        .and_then(|v| Uuid::parse_str(v).ok());
    let metadata: Option<serde_json::Value> = fields
        .get("metadata")
        .and_then(|v| serde_json::from_str(v).ok());

    let actor: Actor = auth.into();
    let mime = if file.content_type.is_empty() {
        "application/octet-stream".to_string()
    } else {
        file.content_type.clone()
    };
    if !mime_allowed(&supported_types(), &mime) {
        return Err(ApiError::BadRequest("file type not supported".into()));
    }
    if file.bytes.len() as u64 > max_file {
        return Err(ApiError::BadRequest(
            "file too large or failed to get file".into(),
        ));
    }

    let total = node_repo::uploads_size(&state.db, actor.user_id).await?;
    if (total as u64) + file.bytes.len() as u64 > max_total {
        return Err(ApiError::BadRequest(
            "total size of uploads exceeds the limit".into(),
        ));
    }

    let id = crate::types::new_id();
    let ext: String = file
        .filename
        .rsplit_once('.')
        .map(|(_, e)| format!(".{}", e))
        .unwrap_or_default();
    let transformed_path = format!("{}{}", id, ext);
    let metadata_json = build_metadata(&mime, &file.filename, &transformed_path, metadata.as_ref());

    let mut name = file.filename.clone();
    if name.len() > 50 {
        // Go slices bytes; keep byte-boundary parity then trim to a char edge.
        name = name.chars().take(50).collect();
    }
    let accessibility = Some(1);

    // Parent permission check mirrors nodes create: disallowed parent → null.
    let mut safe_parent = parent_id;
    if let Some(pid) = safe_parent {
        let parent = node_repo::get_by_id(&state.db, pid).await.unwrap_or(None);
        let allowed = match &parent {
            Some(p) => guard::check_node_action(&state.db, &actor, p, NodeAction::Update)
                .await
                .map(|l| l != guard::PERM_NONE)
                .unwrap_or(false),
            None => false,
        };
        if !allowed {
            safe_parent = None;
        }
    }

    let write = node_repo::NodeWrite {
        parent_id: safe_parent,
        name,
        description: None,
        tags: None,
        role: 4,
        color: None,
        icon: None,
        thumbnail: None,
        theme: None,
        accessibility,
        access: 0,
        display: None,
        node_order: None,
        content: Some(file.filename.clone()),
        content_compiled: Some(transformed_path.clone()),
        size: Some(file.bytes.len() as i64),
        metadata: Some(metadata_json.clone()),
    };
    let now = node_repo::now_ms();
    node_repo::insert(&state.db, id, actor.user_id, &write, now)
        .await
        .map_err(ApiError::from)?;

    let object_key = format!("{}/{}{}", actor.user_id, id, ext);
    storage
        .put_user_object(
            actor.user_id,
            &format!("{}{}", id, ext),
            file.bytes.clone(),
            Some(&mime),
        )
        .await?;
    let _ = object_key;

    let node = node_repo::get_by_id(&state.db, id)
        .await
        .map_err(ApiError::from)?
        .ok_or(ApiError::Internal(Box::new(std::io::Error::other(
            "created node missing",
        ))))?;
    Ok(ApiOk::with_status(
        axum::http::StatusCode::OK,
        serde_json::to_value(&node).unwrap_or(json!(null)),
    ))
}

/// POST /resources/avatar — image-only upload to `{user}/avatar`.
pub async fn upload_avatar(
    State(state): State<AppState>,
    auth: AuthUser,
    multipart: Multipart,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let (max_file, _) = cdn_limits();
    let storage = state
        .storage
        .clone()
        .ok_or(ApiError::BadRequest("minio client not initialized".into()))?;

    let (file, _) = read_upload(multipart).await?;
    let Some(file) = file else {
        tracing::warn!("debug: file field missing after read");
        return Err(ApiError::BadRequest(
            "file too large or failed to get file".into(),
        ));
    };

    let mime = if file.content_type.is_empty() {
        "application/octet-stream".to_string()
    } else {
        file.content_type.clone()
    };
    if !mime_allowed(&supported_images(), &mime) {
        return Err(ApiError::BadRequest("file type not supported".into()));
    }
    if file.bytes.len() as u64 > max_file {
        return Err(ApiError::BadRequest(
            "file too large or failed to get file".into(),
        ));
    }

    storage
        .put_user_object(auth.user_id(), "avatar", file.bytes, Some(&mime))
        .await?;
    Ok(ApiOk::ok(json!("Avatar uploaded successfully")))
}

/// PUT /resources/:id — replace the stored object (write access), update node
/// size/metadata; optional name-only metadata updates when no file is sent.
pub async fn update_file(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    mut multipart: Multipart,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let (max_file, max_total) = cdn_limits();
    let storage = state
        .storage
        .clone()
        .ok_or(ApiError::BadRequest("minio client not initialized".into()))?;

    let resource_id =
        Uuid::parse_str(&id).map_err(|_| ApiError::BadRequest("invalid resource id".into()))?;

    let actor: Actor = auth.into();
    let mut file: Option<UploadedPart> = None;
    let mut fields = std::collections::HashMap::new();
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        tracing::warn!(error = %e, "multipart next_field failed");
        ApiError::BadRequest("file too large or failed to get file".into())
    })? {
        let name = field.name().unwrap_or("").to_string();
        tracing::info!(%name, "multipart field seen");
        match name.as_str() {
            "file" => {
                let filename = field.file_name().unwrap_or("").to_string();
                let content_type = field.content_type().unwrap_or("").to_string();
                let bytes = field.bytes().await.map_err(|e| {
                    tracing::warn!(error = %e, "field bytes read failed");
                    ApiError::BadRequest(format!("file too large or failed to get file: {e}"))
                })?;
                file = Some(UploadedPart {
                    filename,
                    content_type,
                    bytes: bytes.to_vec(),
                });
            }
            name => {
                let value = field
                    .text()
                    .await
                    .map_err(|_| ApiError::BadRequest("failed to read form field".into()))?;
                fields.insert(name.to_string(), value);
            }
        }
    }

    let db_node: NodeRow = node_repo::get_by_id(&state.db, resource_id)
        .await
        .map_err(ApiError::from)?
        .ok_or(ApiError::BadRequest("resource not found".into()))?;

    guard::check_node_action(&state.db, &actor, &db_node, NodeAction::Update)
        .await
        .map_err(|e| ApiError::BadRequest(e.message()))?;

    let now = node_repo::now_ms();
    if let Some(file) = file {
        let mime = if file.content_type.is_empty() {
            "application/octet-stream".to_string()
        } else {
            file.content_type.clone()
        };
        if !mime_allowed(&supported_types(), &mime) {
            return Err(ApiError::BadRequest("file type not supported".into()));
        }
        if file.bytes.len() as u64 > max_file {
            return Err(ApiError::BadRequest(
                "file too large or failed to get file".into(),
            ));
        }

        let current_size = db_node.size.unwrap_or(0);
        let total = node_repo::uploads_size(&state.db, actor.user_id).await?;
        let size_increase = file.bytes.len() as i64 - current_size;
        if (total as i64) + size_increase > max_total as i64 {
            return Err(ApiError::BadRequest(
                "total size of uploads exceeds the limit".into(),
            ));
        }

        // Go: object key from the EXISTING transformed_path (extension kept).
        let existing_path = db_node
            .metadata
            .as_ref()
            .and_then(|m| m.get("transformed_path"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let file_len = file.bytes.len();
        storage
            .put_user_object(actor.user_id, &existing_path, file.bytes, Some(&mime))
            .await?;

        let mut metadata = db_node.metadata.clone().unwrap_or(json!({}));
        if let Some(obj) = metadata.as_object_mut() {
            obj.insert("filetype".into(), json!(mime));
        }

        let size = Some(file_len as i64);
        node_repo::update(
            &state.db,
            resource_id,
            &node_repo::NodeWrite {
                parent_id: db_node.parent_id,
                name: db_node.name.clone(),
                description: db_node.description.clone(),
                tags: db_node.tags.clone(),
                role: db_node.role,
                color: db_node.color,
                icon: db_node.icon.clone(),
                thumbnail: db_node.thumbnail.clone(),
                theme: db_node.theme.clone(),
                accessibility: db_node.accessibility,
                access: db_node.access,
                display: db_node.display,
                node_order: db_node.node_order,
                content: db_node.content.clone(),
                content_compiled: db_node.content_compiled.clone(),
                size,
                metadata: Some(metadata),
            },
            now,
        )
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "file update failed");
            ApiError::BadRequest("failed to update file in storage".into())
        })?;
    }

    let _ = fields; // name-only updates flow through PATCH /nodes/{id} (Go parity note)

    let updated = node_repo::get_by_id(&state.db, resource_id)
        .await
        .map_err(ApiError::from)?
        .ok_or(ApiError::NotFound("not found".into()))?;
    Ok(ApiOk::ok(json!(updated)))
}

#[derive(Debug, Deserialize)]
pub struct DownloadQuery {
    #[serde(default)]
    pub download: bool,
}

/// S3 object location resolved from a resource node: the storage key relative
/// to the `{user_id}/` prefix, the MIME type, and the original filename.
struct ObjectRef {
    key: String,
    mime: String,
    filename: String,
}

fn object_ref(node: &NodeRow) -> Option<ObjectRef> {
    let meta = node.metadata.as_ref()?;
    let key = meta.get("transformed_path")?.as_str()?;
    if key.is_empty() {
        return None;
    }
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
    let range = headers
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .filter(|v| v.starts_with("bytes="));
    let (stream, len, content_range, etag) = storage
        .get_user_object_stream(user_id, &obj.key, range)
        .await?;
    let body = Body::from_stream(tokio_util::io::ReaderStream::new(stream.into_async_read()));
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
