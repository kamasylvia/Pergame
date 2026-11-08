//! /resources routes — file upload, avatar upload, file update
//! (Go controllers/resources.controller.go + services/resource.service.go
//! parity).

use axum::extract::{Multipart, Path, State};
use serde_json::json;

use crate::auth::guard::{self, Actor, NodeAction};
use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::models::NodeRow;
use crate::repo::nodes as node_repo;
use crate::response::ApiOk;
use crate::AppState;

const DEFAULT_TYPES: &[&str] = &[
    "image/png", "image/jpeg", "image/gif", "image/bmp", "image/webp", "image/avif",
    "image/svg+xml", "image/vnd.microsoft.icon",
    "video/mp4", "video/x-msvideo", "video/quicktime", "video/x-ms-wmv", "video/x-flv",
    "video/x-matroska", "video/webm",
    "audio/mpeg", "audio/wav", "audio/ogg", "audio/flac", "audio/aac", "audio/webm",
    "application/pdf", "application/msword",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/vnd.ms-excel",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "application/vnd.ms-powerpoint",
    "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    "text/plain", "text/csv",
    "application/zip", "application/x-rar-compressed", "application/x-tar",
    "application/gzip", "application/x-bzip2",
];

const IMAGE_TYPES: &[&str] = &[
    "image/png", "image/jpeg", "image/gif", "image/bmp", "image/webp", "image/avif",
    "image/svg+xml", "image/vnd.microsoft.icon",
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
async fn read_upload(mut multipart: Multipart) -> Result<(Option<UploadedPart>, std::collections::HashMap<String, String>), ApiError> {
    let mut file = None;
    let mut fields = std::collections::HashMap::new();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| ApiError::BadRequest("file too large or failed to get file".into()))?
    {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "file" => {
                let filename = field.file_name().unwrap_or("").to_string();
                let content_type = field.content_type().unwrap_or("").to_string();
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|_| ApiError::BadRequest("file too large or failed to get file".into()))?;
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

fn build_metadata(filetype: &str, original_path: &str, transformed_path: &str, extra: Option<&serde_json::Value>) -> serde_json::Value {
    let mut base = json!({
        "filetype": filetype,
        "original_path": original_path,
        "transformed_path": transformed_path,
    });
    // Go maps.Copy(metadata_final, metadata): user-provided keys override.
    if let (Some(base_obj), Some(extra_obj)) = (base.as_object_mut(), extra.and_then(|v| v.as_object())) {
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
        return Err(ApiError::BadRequest("file too large or failed to get file".into()));
    };

    let parent_id: Option<i64> = fields
        .get("parent_id")
        .and_then(|v| crate::types::Snowflake::parse(v))
        .map(|s| s.as_i64());
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
        return Err(ApiError::BadRequest("file too large or failed to get file".into()));
    }

    let total = node_repo::uploads_size(&state.db, actor.user_id).await?;
    if (total as u64) + file.bytes.len() as u64 > max_total {
        return Err(ApiError::BadRequest("total size of uploads exceeds the limit".into()));
    }

    let id = state.snowflake.generate().as_i64();
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
        .put_user_object(actor.user_id, &format!("{}{}", id, ext), file.bytes.clone(), Some(&mime))
        .await?;
    let _ = object_key;

    let node = node_repo::get_by_id(&state.db, id)
        .await
        .map_err(ApiError::from)?
        .ok_or(ApiError::Internal(Box::new(std::io::Error::other("created node missing"))))?;
    Ok(ApiOk::with_status(axum::http::StatusCode::OK, serde_json::to_value(&node).unwrap_or(json!(null))))
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
        return Err(ApiError::BadRequest("file too large or failed to get file".into()));
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
        return Err(ApiError::BadRequest("file too large or failed to get file".into()));
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

    let resource_id = crate::types::Snowflake::parse(&id)
        .ok_or(ApiError::BadRequest("invalid resource id".into()))?
        .as_i64();

    let actor: Actor = auth.into();
    let mut file: Option<UploadedPart> = None;
    let mut fields = std::collections::HashMap::new();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| ApiError::BadRequest("file too large or failed to get file".into()))?
    {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "file" => {
                let filename = field.file_name().unwrap_or("").to_string();
                let content_type = field.content_type().unwrap_or("").to_string();
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|_| ApiError::BadRequest("file too large or failed to get file".into()))?;
                file = Some(UploadedPart { filename, content_type, bytes: bytes.to_vec() });
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
            return Err(ApiError::BadRequest("file too large or failed to get file".into()));
        }

        let current_size = db_node.size.unwrap_or(0);
        let total = node_repo::uploads_size(&state.db, actor.user_id).await?;
        let size_increase = file.bytes.len() as i64 - current_size;
        if (total as i64) + size_increase > max_total as i64 {
            return Err(ApiError::BadRequest("total size of uploads exceeds the limit".into()));
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
