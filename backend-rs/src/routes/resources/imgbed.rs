//! Image-host admin surface: status, local media migration, external-link
//! import. All operations are scoped to the signed-in user (self-service).

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use axum::Json;
use axum::extract::State;
use serde::Deserialize;
use serde_json::json;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::models::NodeRow;
use crate::repo::nodes as node_repo;
use crate::response::ApiOk;

#[derive(Debug, Deserialize, Default)]
pub struct TransferInput {
    /// `["image", "video"]`; empty = both.
    #[serde(default)]
    pub types: Vec<String>,
    /// Single resource (migrate) or document (import); empty = whole library.
    #[serde(default)]
    pub node_id: Option<uuid::Uuid>,
}

fn wants(mime: &str, types: &[String]) -> bool {
    let image = types.is_empty() || types.iter().any(|t| t == "image");
    let video = types.is_empty() || types.iter().any(|t| t == "video");
    (image && mime.starts_with("image/")) || (video && mime.starts_with("video/"))
}

fn metadata_str(node: &NodeRow, key: &str) -> String {
    node.metadata
        .as_ref()
        .and_then(|m| m.get(key))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn write_from(
    node: &NodeRow,
    metadata: serde_json::Value,
    content: Option<String>,
    content_compiled: Option<String>,
) -> node_repo::NodeWrite {
    node_repo::NodeWrite {
        parent_id: node.parent_id,
        name: node.name.clone(),
        description: node.description.clone(),
        tags: node.tags.clone(),
        role: node.role,
        color: node.color,
        icon: node.icon.clone(),
        thumbnail: node.thumbnail.clone(),
        theme: node.theme.clone(),
        accessibility: node.accessibility,
        access: node.access,
        display: node.display,
        node_order: node.node_order,
        content,
        content_compiled,
        size: node.size,
        metadata: Some(metadata),
    }
}

/// GET /resources/imgbed — adapter availability + the caller's private-note
/// opt-in (frontend uses this to show the panel/toggle).
pub async fn status(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let private = crate::repo::settings_stats::get_settings(&state.db, auth.user_id())
        .await
        .ok()
        .flatten()
        .and_then(|s| s.general.get("imgbedPrivate").and_then(|v| v.as_bool()))
        .unwrap_or(false);
    Ok(ApiOk::ok(json!({
        "enabled": crate::imgbed::Imgbed::from_config().is_some(),
        "private_notes": private,
    })))
}

/// POST /resources/imgbed/migrate — copy the caller's existing S3 media to
/// the image host; S3 objects stay as backup.
pub async fn migrate_local(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<TransferInput>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let bed = crate::imgbed::Imgbed::from_config()
        .ok_or(ApiError::BadRequest("image host not configured".into()))?;
    let storage = state
        .storage
        .clone()
        .ok_or(ApiError::BadRequest("minio client not initialized".into()))?;
    let nodes = node_repo::list_all(&state.db, auth.user_id())
        .await
        .map_err(ApiError::from)?;

    let mut uploaded: HashMap<String, String> = HashMap::new();
    let mut migrated = 0usize;
    let mut failed = Vec::new();
    let mut bytes = 0i64;
    for node in nodes {
        if let Some(only) = input.node_id
            && node.id != only
        {
            continue;
        }
        if node.role != 4 || !metadata_str(&node, "imgbed_url").is_empty() {
            continue;
        }
        let mime = metadata_str(&node, "filetype");
        if !wants(&mime, &input.types) {
            continue;
        }
        let sha = metadata_str(&node, "content_sha256");
        let path = metadata_str(&node, "transformed_path");
        if sha.is_empty() && (path.is_empty() || path.starts_with("http")) {
            continue;
        }
        let cache_key = if sha.is_empty() {
            format!("{}:{path}", node.user_id)
        } else {
            format!("blob:{sha}")
        };
        let url = if let Some(url) = uploaded.get(&cache_key) {
            url.clone()
        } else {
            let data = match (sha.is_empty(), sha.as_str()) {
                (false, sha) => storage.get_blob(sha).await,
                _ => storage.get_user_object(auth.user_id(), &path).await,
            };
            let data = match data {
                Ok(d) => d,
                Err(e) => {
                    failed.push(json!({"id": node.id, "error": e.message()}));
                    continue;
                }
            };
            let filename = {
                let name = metadata_str(&node, "original_path");
                if name.is_empty() {
                    node.name.clone()
                } else {
                    name
                }
            };
            match bed.upload(&data, &filename, &mime).await {
                Ok(url) => {
                    uploaded.insert(cache_key, url.clone());
                    bytes += data.len() as i64;
                    url
                }
                Err(e) => {
                    failed.push(json!({"id": node.id, "error": e.message()}));
                    continue;
                }
            }
        };
        let mut metadata = node.metadata.clone().unwrap_or(json!({}));
        if let Some(obj) = metadata.as_object_mut() {
            obj.insert("imgbed_url".into(), json!(url));
            obj.insert("transformed_path".into(), json!(url));
        }
        node_repo::update(
            &state.db,
            node.id,
            &write_from(
                &node,
                metadata,
                node.content.clone(),
                node.content_compiled.clone(),
            ),
            node_repo::now_ms(),
        )
        .await
        .map_err(ApiError::from)?;
        migrated += 1;
    }
    Ok(ApiOk::ok(json!({
        "migrated": migrated,
        "bytes": bytes,
        "failed": failed,
    })))
}

fn url_patterns() -> &'static [regex::Regex] {
    static PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
        [
            r#"!\[[^\]]*\]\((https?://[^)\s]+)\)"#,
            r#"<img[^>]+src=["'](https?://[^"']+)["']"#,
            r#"<video[^>]+src=["'](https?://[^"']+)["']"#,
        ]
        .iter()
        .filter_map(|p| regex::Regex::new(p).ok())
        .collect()
    });
    &PATTERNS
}

fn external_urls(content: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for re in url_patterns() {
        for cap in re.captures_iter(content) {
            if let Some(url) = cap.get(1) {
                let url = url.as_str().to_string();
                if seen.insert(url.clone()) {
                    out.push(url);
                }
            }
        }
    }
    out
}

/// POST /resources/imgbed/import — download external images/videos in the
/// caller's documents, upload them to the image host, rewrite the markdown.
pub async fn import_external(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<TransferInput>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let bed = crate::imgbed::Imgbed::from_config()
        .ok_or(ApiError::BadRequest("image host not configured".into()))?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| ApiError::Internal(Box::new(e)))?;
    let nodes = node_repo::list_all(&state.db, auth.user_id())
        .await
        .map_err(ApiError::from)?;

    let mut cache: HashMap<String, String> = HashMap::new();
    let mut failed = Vec::new();
    let mut imported = 0usize;
    let mut touched = 0usize;
    for node in nodes {
        if let Some(only) = input.node_id
            && node.id != only
        {
            continue;
        }
        if node.role != 3 {
            continue;
        }
        // `list_all` omits content columns; fetch the full row for rewriting.
        let Some(node) = node_repo::get_by_id(&state.db, node.id)
            .await
            .map_err(ApiError::from)?
        else {
            continue;
        };
        let Some(content) = node.content.as_deref() else {
            continue;
        };
        let urls = external_urls(content);
        if urls.is_empty() {
            continue;
        }
        let mut next = content.to_string();
        let mut replacements: Vec<(String, String)> = Vec::new();
        let mut node_changed = false;
        for url in urls {
            let replacement = if let Some(done) = cache.get(&url) {
                Some(done.clone())
            } else {
                match download_and_upload(&client, &bed, &url, &input.types).await {
                    Ok(Some(done)) => {
                        cache.insert(url.clone(), done.clone());
                        imported += 1;
                        Some(done)
                    }
                    Ok(None) => None,
                    Err(e) => {
                        failed.push(json!({"url": url, "error": e}));
                        None
                    }
                }
            };
            if let Some(done) = replacement
                && done != url
            {
                next = next.replace(&url, &done);
                replacements.push((url, done));
                node_changed = true;
            }
        }
        if !node_changed {
            continue;
        }
        let compiled = node.content_compiled.as_ref().map(|c| {
            let mut out = c.clone();
            for (old, new) in &replacements {
                out = out.replace(old, new);
            }
            out
        });
        node_repo::update(
            &state.db,
            node.id,
            &write_from(
                &node,
                node.metadata.clone().unwrap_or(json!({})),
                Some(next),
                compiled,
            ),
            node_repo::now_ms(),
        )
        .await
        .map_err(ApiError::from)?;
        touched += 1;
    }
    Ok(ApiOk::ok(json!({
        "imported": imported,
        "documents": touched,
        "failed": failed,
    })))
}

async fn download_and_upload(
    client: &reqwest::Client,
    bed: &crate::imgbed::Imgbed,
    url: &str,
    types: &[String],
) -> Result<Option<String>, String> {
    let parsed = url::Url::parse(url).map_err(|e| e.to_string())?;
    crate::routes::plugins::ensure_public_target(&parsed)
        .await
        .map_err(|e| e.message())?;
    let resp = client.get(url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let mime = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    if !wants(&mime, types) {
        return Ok(None);
    }
    if let Some(len) = resp.content_length()
        && len > 100 * 1024 * 1024
    {
        return Err("file too large".into());
    }
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    if bytes.len() > 100 * 1024 * 1024 {
        return Err("file too large".into());
    }
    let filename = parsed
        .path_segments()
        .and_then(|mut s| s.next_back())
        .filter(|s| !s.is_empty())
        .unwrap_or("external");
    bed.upload(&bytes, filename, &mime)
        .await
        .map(Some)
        .map_err(|e| e.message())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_by_media_type() {
        assert!(wants("image/png", &[]));
        assert!(wants("video/mp4", &[]));
        assert!(wants("image/png", &["image".into()]));
        assert!(!wants("video/mp4", &["image".into()]));
        assert!(!wants("application/pdf", &[]));
    }

    #[test]
    fn extracts_markdown_and_html_urls() {
        let text = "![a](https://x/a.png)\n<img src=\"https://x/b.png\">\n<video src='https://x/c.mp4'></video>\n![dup](https://x/a.png)";
        let urls = external_urls(text);
        assert_eq!(urls.len(), 3);
    }
}
