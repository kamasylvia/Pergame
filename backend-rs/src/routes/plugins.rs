//! Plugin registry routes (PS-D).
//!
//! Two audiences:
//! - **admins** install, enable and remove plugin archives (`/admin/plugins`);
//! - **every user** lists what they may load, approves capabilities per
//!   plugin, and reads/writes their own plugin settings.
//!
//! The manifest is validated here before anything is stored (the web client
//! repeats the check, but a hostile archive never reaches S3). Plugin code is
//! served from the bucket under `plugins/<id>/<version>/`, so an install is
//! one row plus a prefix; removing the plugin drops both.

use std::collections::HashMap;

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Multipart, Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::plugins_manifest::{self, PluginManifest};
use crate::repo::plugins as repo;
use crate::response::ApiOk;

/// Largest accepted plugin archive (compressed).
const MAX_ARCHIVE_BYTES: usize = 4 * 1024 * 1024;
/// Largest accepted single file inside an archive (plugin entries are small).
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
/// Cap on entries: a plugin is a handful of files, not a bundle.
const MAX_ARCHIVE_ENTRIES: usize = 64;
/// Longest plugin KV key.
const MAX_KV_KEY: usize = 128;
/// Largest serialized KV value.
const MAX_KV_VALUE_BYTES: usize = 64 * 1024;

fn require_admin(auth: &AuthUser) -> Result<(), ApiError> {
    if auth.role == 2 {
        Ok(())
    } else {
        Err(ApiError::Unauthorized("Unauthorized".into()))
    }
}

/// Reject archive paths that escape the plugin root or hide in dot dirs.
fn is_safe_archive_path(name: &str) -> bool {
    if name.is_empty() || name.len() > 200 || name.contains('\\') {
        return false;
    }
    name.split('/').all(|segment| {
        !segment.is_empty()
            && segment != "."
            && segment != ".."
            && !segment.starts_with('.')
            && segment
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    })
}

/// Capabilities one user granted, filtered to what the manifest declares.
async fn effective_grants(
    state: &AppState,
    user_id: uuid::Uuid,
    plugin: &repo::PluginRow,
) -> Result<Vec<String>, ApiError> {
    let granted = repo::grants_for(&state.db, user_id, &plugin.id)
        .await
        .map_err(ApiError::from)?;
    Ok(granted
        .into_iter()
        .filter(|cap| plugin.capabilities.contains(cap))
        .collect())
}

/// GET /plugins — installed and enabled plugins for the caller.
///
/// `granted` drives the loader: the client only hands a plugin the
/// capabilities the user approved, and skips plugins with nothing granted.
pub async fn list_plugins(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let rows = repo::list(&state.db).await.map_err(ApiError::from)?;
    let mut out = Vec::new();
    for row in rows.into_iter().filter(|row| row.enabled) {
        let granted = effective_grants(&state, auth.user_id(), &row).await?;
        out.push(json!({
            "id": row.id,
            "name": row.name,
            "version": row.version,
            "apiVersion": row.api_version,
            "entry": row.entry,
            "capabilities": row.capabilities,
            "granted": granted,
            "manifest": row.manifest,
            "entry_url": format!("/api/plugins/{}/entry", row.id),
        }));
    }
    Ok(ApiOk::ok(json!(out)))
}

/// GET /admin/plugins — every installed plugin, disabled ones included.
///
/// The settings page needs the full table (admins enable, disable and
/// uninstall); the per-user loader uses `GET /plugins` instead.
pub async fn admin_list_plugins(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    let rows = repo::list(&state.db).await.map_err(ApiError::from)?;
    let mut out = Vec::new();
    for row in rows {
        let granted = repo::grants_for(&state.db, auth.user_id(), &row.id)
            .await
            .map_err(ApiError::from)?;
        out.push(json!({
            "id": row.id,
            "name": row.name,
            "version": row.version,
            "enabled": row.enabled,
            "capabilities": row.capabilities,
            "granted": granted,
            "manifest": row.manifest,
        }));
    }
    Ok(ApiOk::ok(json!(out)))
}

/// GET /plugins/{id}/entry — the plugin's ES module, served same-origin so a
/// dynamic `import()` carries the session cookie.
pub async fn plugin_entry(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let row = repo::get(&state.db, &id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::NotFound("plugin not found".into()))?;
    if !row.enabled {
        return Err(ApiError::NotFound("plugin is disabled".into()));
    }
    let storage = state.storage.as_ref().ok_or_else(|| {
        ApiError::Internal(Box::new(std::io::Error::other("storage not configured")))
    })?;
    let key = format!("{}{}", row.artifact_prefix, row.entry);
    let bytes = storage.get_object_bytes(&key).await?;
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        bytes,
    )
        .into_response())
}

/// GET /plugins/{id}/asset/{*path} — styles and other files next to the entry.
pub async fn plugin_asset(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path((id, path)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    if !is_safe_archive_path(&path) {
        return Err(ApiError::BadRequest("invalid asset path".into()));
    }
    let row = repo::get(&state.db, &id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::NotFound("plugin not found".into()))?;
    let storage = state.storage.as_ref().ok_or_else(|| {
        ApiError::Internal(Box::new(std::io::Error::other("storage not configured")))
    })?;
    let key = format!("{}{}", row.artifact_prefix, path);
    let bytes = storage.get_object_bytes(&key).await?;
    let content_type = if path.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if path.ends_with(".json") {
        "application/json"
    } else {
        "application/octet-stream"
    };
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        bytes,
    )
        .into_response())
}

/// One file taken out of an uploaded archive.
struct ArchiveFile {
    path: String,
    bytes: Vec<u8>,
}

/// Read the `file` part of a plugin upload (a zip).
async fn read_archive(mut multipart: Multipart) -> Result<Vec<u8>, ApiError> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| ApiError::BadRequest("failed to read upload".into()))?
    {
        if field.name() == Some("file") {
            let bytes = field
                .bytes()
                .await
                .map_err(|e| ApiError::BadRequest(format!("upload too large: {e}")))?;
            if bytes.len() > MAX_ARCHIVE_BYTES {
                return Err(ApiError::BadRequest(format!(
                    "archive exceeds {MAX_ARCHIVE_BYTES} bytes"
                )));
            }
            return Ok(bytes.to_vec());
        }
    }
    Err(ApiError::BadRequest("missing file field".into()))
}

/// Expand a zip into validated files (paths checked, sizes capped).
fn unpack_archive(bytes: &[u8]) -> Result<Vec<ArchiveFile>, ApiError> {
    let reader = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader)
        .map_err(|e| ApiError::BadRequest(format!("not a zip archive: {e}")))?;
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err(ApiError::BadRequest(format!(
            "archive has more than {MAX_ARCHIVE_ENTRIES} entries"
        )));
    }
    let mut files = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| ApiError::BadRequest(format!("cannot read archive entry: {e}")))?;
        if entry.is_dir() {
            continue;
        }
        // zip 9 returns a Result here (older versions gave &str).
        let name = entry
            .name()
            .map_err(|e| ApiError::BadRequest(format!("archive entry without a name: {e}")))?
            .to_string();
        // Archives built on macOS may carry a single wrapping directory; strip
        // it so `my-plugin/manifest.json` works as-is.
        let name = name.strip_prefix("./").unwrap_or(&name).to_string();
        if !is_safe_archive_path(&name) {
            return Err(ApiError::BadRequest(format!(
                "unsafe path in archive: {name}"
            )));
        }
        if entry.size() > MAX_FILE_BYTES {
            return Err(ApiError::BadRequest(format!(
                "{name} exceeds {MAX_FILE_BYTES} bytes"
            )));
        }
        let mut buffer = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut buffer)
            .map_err(|e| ApiError::BadRequest(format!("cannot read {name}: {e}")))?;
        files.push(ArchiveFile {
            path: name,
            bytes: buffer,
        });
    }
    Ok(files)
}

/// Drop a shared wrapping directory (`my-plugin/manifest.json` → `manifest.json`).
fn strip_common_root(files: Vec<ArchiveFile>) -> Vec<ArchiveFile> {
    let mut parts = files.iter().map(|f| f.path.split('/').next().unwrap_or(""));
    let Some(first) = parts.next() else {
        return files;
    };
    if first.is_empty() || !parts.all(|part| part == first) {
        return files;
    }
    files
        .into_iter()
        .map(|f| ArchiveFile {
            path: f
                .path
                .split_once('/')
                .map(|(_, rest)| rest.to_string())
                .unwrap_or(f.path),
            bytes: f.bytes,
        })
        .collect()
}

/// POST /admin/plugins — install or replace a plugin from a zip archive.
pub async fn install_plugin(
    State(state): State<AppState>,
    auth: AuthUser,
    multipart: Multipart,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    let bytes = read_archive(multipart).await?;
    let files = strip_common_root(unpack_archive(&bytes)?);
    let manifest_bytes = files
        .iter()
        .find(|f| f.path == "manifest.json")
        .map(|f| f.bytes.clone())
        .ok_or_else(|| ApiError::BadRequest("archive has no manifest.json".into()))?;
    let raw: serde_json::Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|e| ApiError::BadRequest(format!("manifest.json is not valid JSON: {e}")))?;
    let manifest: PluginManifest = plugins_manifest::validate(&raw).map_err(|errors| {
        ApiError::BadRequest(format!("invalid manifest: {}", errors.join("; ")))
    })?;
    if !files.iter().any(|f| f.path == manifest.entry) {
        return Err(ApiError::BadRequest(format!(
            "entry '{}' is missing from the archive",
            manifest.entry
        )));
    }

    let storage = state.storage.as_ref().ok_or_else(|| {
        ApiError::Internal(Box::new(std::io::Error::other("storage not configured")))
    })?;
    let prefix = format!("plugins/{}/{}/", manifest.id, manifest.version);
    // Replacing a version leaves the previous prefix behind; drop it first so
    // an uninstall never resurrects stale files.
    if let Some(existing) = repo::get(&state.db, &manifest.id)
        .await
        .map_err(ApiError::from)?
        && existing.artifact_prefix != prefix
    {
        storage.delete_prefix(&existing.artifact_prefix).await;
    }
    storage.delete_prefix(&prefix).await;

    for file in &files {
        let content_type = if file.path.ends_with(".mjs") {
            "text/javascript"
        } else if file.path.ends_with(".css") {
            "text/css"
        } else if file.path.ends_with(".json") {
            "application/json"
        } else {
            "application/octet-stream"
        };
        storage
            .put_object(
                &format!("{prefix}{}", file.path),
                file.bytes.clone(),
                Some(content_type),
            )
            .await?;
    }

    repo::upsert(
        &state.db,
        &manifest.id,
        &manifest.name,
        &manifest.version,
        &manifest.api_version,
        &manifest.entry,
        &manifest.capabilities,
        &raw,
        &prefix,
        auth.user_id(),
        crate::repo::nodes::now_ms(),
    )
    .await
    .map_err(ApiError::from)?;

    crate::mcp::audit(
        &state.db,
        Some(auth.user_id()),
        "plugin.install",
        &format!(
            "installed {} {} ({} files, caps: {})",
            manifest.id,
            manifest.version,
            files.len(),
            manifest.capabilities.join(",")
        ),
    )
    .await;

    Ok(ApiOk::with_status(
        StatusCode::CREATED,
        json!({
            "id": manifest.id,
            "name": manifest.name,
            "version": manifest.version,
            "capabilities": manifest.capabilities,
            "files": files.len(),
        }),
    ))
}

#[derive(Debug, Deserialize)]
/// PATCH /admin/plugins/{id} body.
pub struct PluginPatch {
    pub enabled: bool,
}

/// PATCH /admin/plugins/{id} — enable or disable an installed plugin.
pub async fn patch_plugin(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<PluginPatch>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    let updated = repo::set_enabled(&state.db, &id, body.enabled, crate::repo::nodes::now_ms())
        .await
        .map_err(ApiError::from)?;
    if !updated {
        return Err(ApiError::NotFound("plugin not found".into()));
    }
    crate::mcp::audit(
        &state.db,
        Some(auth.user_id()),
        "plugin.enable",
        &format!("plugin {id} enabled={}", body.enabled),
    )
    .await;
    Ok(ApiOk::ok(json!({ "id": id, "enabled": body.enabled })))
}

/// DELETE /admin/plugins/{id} — uninstall (artifacts, grants and KV go too).
pub async fn delete_plugin(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    let row = repo::get(&state.db, &id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::NotFound("plugin not found".into()))?;
    repo::delete(&state.db, &id).await.map_err(ApiError::from)?;
    if let Some(storage) = state.storage.as_ref() {
        storage.delete_prefix(&row.artifact_prefix).await;
    }
    crate::mcp::audit(
        &state.db,
        Some(auth.user_id()),
        "plugin.uninstall",
        &format!("uninstalled plugin {id}"),
    )
    .await;
    Ok(ApiOk::ok(json!({ "id": id, "deleted": true })))
}

#[derive(Debug, Deserialize)]
/// PUT /plugins/{id}/grants body.
pub struct GrantsInput {
    #[serde(default)]
    pub capabilities: Vec<String>,
}

/// PUT /plugins/{id}/grants — record the caller's approved capabilities.
///
/// Only capabilities the manifest declares can be granted; anything else is
/// dropped rather than rejected, so a manifest that lost a capability does not
/// strand a stored grant.
pub async fn put_grants(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<GrantsInput>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let row = repo::get(&state.db, &id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::NotFound("plugin not found".into()))?;
    let mut granted: Vec<String> = Vec::new();
    for cap in body.capabilities {
        let cap = cap.trim().to_string();
        if row.capabilities.contains(&cap) && !granted.contains(&cap) {
            granted.push(cap);
        }
    }
    repo::set_grants(
        &state.db,
        auth.user_id(),
        &id,
        &granted,
        crate::repo::nodes::now_ms(),
    )
    .await
    .map_err(ApiError::from)?;
    crate::mcp::audit(
        &state.db,
        Some(auth.user_id()),
        "plugin.grant",
        &format!("plugin {id} granted: {}", granted.join(",")),
    )
    .await;
    Ok(ApiOk::ok(json!({ "id": id, "granted": granted })))
}

/// Ensure the caller may use a plugin's settings namespace.
async fn require_granted(
    state: &AppState,
    auth: &AuthUser,
    id: &str,
) -> Result<repo::PluginRow, ApiError> {
    let row = repo::get(&state.db, id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::NotFound("plugin not found".into()))?;
    if !row.enabled {
        return Err(ApiError::NotFound("plugin is disabled".into()));
    }
    let granted = effective_grants(state, auth.user_id(), &row).await?;
    if granted.is_empty() {
        return Err(ApiError::Forbidden("plugin is not approved".into()));
    }
    Ok(row)
}

#[derive(Debug, Deserialize)]
/// POST /plugins/{id}/proxy body.
pub struct ProxyInput {
    pub url: String,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
}

/// Reject targets that could reach the instance's own network (SSRF).
///
/// A plugin may only proxy hosts its manifest names, and even then the host
/// must resolve to a public address: `net:*` is not a licence to probe
/// `localhost`, RFC1918 ranges or the link-local metadata service.
async fn ensure_public_target(url: &url::Url) -> Result<(), ApiError> {
    if url.scheme() != "http" && url.scheme() != "https" {
        return Err(ApiError::BadRequest("proxy supports http(s) only".into()));
    }
    let Some(host) = url.host_str() else {
        return Err(ApiError::BadRequest("proxy target has no host".into()));
    };
    // IP literals: check directly (no DNS to resolve).
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        return if is_private_ip(ip) {
            Err(ApiError::BadRequest(
                "proxy target is not a public address".into(),
            ))
        } else {
            Ok(())
        };
    }
    if host.eq_ignore_ascii_case("localhost")
        || host.ends_with(".local")
        || host.ends_with(".internal")
    {
        return Err(ApiError::BadRequest(
            "proxy target is not a public address".into(),
        ));
    }
    let addrs = tokio::net::lookup_host((host, url.port_or_known_default().unwrap_or(80)))
        .await
        .map_err(|_| ApiError::BadRequest("cannot resolve proxy target".into()))?;
    let mut saw_any = false;
    for addr in addrs {
        saw_any = true;
        if is_private_ip(addr.ip()) {
            return Err(ApiError::BadRequest(
                "proxy target is not a public address".into(),
            ));
        }
    }
    if !saw_any {
        return Err(ApiError::BadRequest("cannot resolve proxy target".into()));
    }
    Ok(())
}

/// Loopback, private, link-local and unique-local ranges.
fn is_private_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.octets()[0] == 100 && (64..=127).contains(&v4.octets()[1]) // CGNAT
        }
        std::net::IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
                // IPv4-mapped: re-check the embedded address.
                || v6.to_ipv4_mapped().is_some_and(|v4| is_private_ip(std::net::IpAddr::V4(v4)))
        }
    }
}

/// POST /plugins/{id}/proxy — server-side fetch for a plugin (breaks CORS).
///
/// This is the replacement for "run a browser automation service next to the
/// instance": article pages are fetched here, with the plugin's declared
/// domain scope as the allow-list and a hard size/time cap. No cookies are
/// borrowed from the browser — a plugin that needs a login state has to ask
/// the user for it and send it explicitly.
pub async fn proxy_fetch(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<ProxyInput>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let row = require_granted(&state, &auth, &id).await?;
    let url =
        url::Url::parse(&body.url).map_err(|_| ApiError::BadRequest("invalid proxy url".into()))?;
    if !crate::plugins_manifest::net_allows(&row.capabilities, &url) {
        return Err(ApiError::Forbidden(
            "the plugin's capabilities do not cover this host".into(),
        ));
    }
    ensure_public_target(&url).await?;

    let method = body.method.as_deref().unwrap_or("GET").to_ascii_uppercase();
    if method != "GET" && method != "POST" {
        return Err(ApiError::BadRequest(
            "proxy supports GET and POST only".into(),
        ));
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .no_proxy()
        .build()
        .map_err(|e| ApiError::Internal(Box::new(std::io::Error::other(e.to_string()))))?;
    let mut request = client.request(
        reqwest::Method::from_bytes(method.as_bytes())
            .map_err(|_| ApiError::BadRequest("invalid method".into()))?,
        url.clone(),
    );
    if let Some(payload) = &body.body {
        request = request
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(payload.clone());
    }
    let response = request
        .send()
        .await
        .map_err(|e| ApiError::BadRequest(format!("proxy request failed: {}", e.without_url())))?;
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("text/html")
        .to_string();
    let bytes = response
        .bytes()
        .await
        .map_err(|e| ApiError::BadRequest(format!("proxy read failed: {}", e.without_url())))?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err(ApiError::BadRequest("proxy response too large".into()));
    }
    let text = String::from_utf8_lossy(&bytes).to_string();

    crate::mcp::audit(
        &state.db,
        Some(auth.user_id()),
        "plugin.proxy",
        &format!(
            "plugin {id} proxied {} ({status}, {} bytes)",
            url.host_str().unwrap_or("?"),
            bytes.len()
        ),
    )
    .await;

    Ok(ApiOk::ok(json!({
        "status": status,
        "content_type": content_type,
        "body": text,
        "url": url.to_string(),
    })))
}

/// GET /plugins/{id}/kv — the caller's whole settings namespace.
pub async fn list_kv(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_granted(&state, &auth, &id).await?;
    let rows = repo::kv_list(&state.db, auth.user_id(), &id)
        .await
        .map_err(ApiError::from)?;
    let map: HashMap<String, serde_json::Value> = rows.into_iter().collect();
    Ok(ApiOk::ok(json!(map)))
}

/// PUT /plugins/{id}/kv/{key} — write one settings entry.
pub async fn put_kv(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((id, key)): Path<(String, String)>,
    body: Bytes,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_granted(&state, &auth, &id).await?;
    if key.is_empty() || key.len() > MAX_KV_KEY {
        return Err(ApiError::BadRequest(format!(
            "key must be 1-{MAX_KV_KEY} characters"
        )));
    }
    if body.len() > MAX_KV_VALUE_BYTES {
        return Err(ApiError::BadRequest("value too large".into()));
    }
    let value: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|e| ApiError::BadRequest(format!("value must be JSON: {e}")))?;
    repo::kv_set(
        &state.db,
        auth.user_id(),
        &id,
        &key,
        &value,
        crate::repo::nodes::now_ms(),
    )
    .await
    .map_err(ApiError::from)?;
    Ok(ApiOk::ok(json!({ "key": key })))
}

/// DELETE /plugins/{id}/kv/{key} — drop one settings entry.
pub async fn delete_kv(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((id, key)): Path<(String, String)>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_granted(&state, &auth, &id).await?;
    let deleted = repo::kv_delete(&state.db, auth.user_id(), &id, &key)
        .await
        .map_err(ApiError::from)?;
    Ok(ApiOk::ok(json!({ "key": key, "deleted": deleted })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_paths_reject_traversal_and_dotfiles() {
        assert!(is_safe_archive_path("main.mjs"));
        assert!(is_safe_archive_path("styles/theme.css"));
        for bad in [
            "../x.mjs",
            "a/../../b.mjs",
            "/abs.mjs",
            ".hidden/x",
            "a\\b",
            "",
        ] {
            assert!(!is_safe_archive_path(bad), "{bad}");
        }
    }

    #[test]
    fn strip_common_root_unwraps_a_single_directory() {
        let files = vec![
            ArchiveFile {
                path: "my-plugin/manifest.json".into(),
                bytes: vec![],
            },
            ArchiveFile {
                path: "my-plugin/main.mjs".into(),
                bytes: vec![],
            },
        ];
        let stripped = strip_common_root(files);
        assert_eq!(stripped[0].path, "manifest.json");
        assert_eq!(stripped[1].path, "main.mjs");
    }

    #[test]
    fn strip_common_root_keeps_flat_archives() {
        let files = vec![
            ArchiveFile {
                path: "manifest.json".into(),
                bytes: vec![],
            },
            ArchiveFile {
                path: "main.mjs".into(),
                bytes: vec![],
            },
        ];
        let stripped = strip_common_root(files);
        assert_eq!(stripped[0].path, "manifest.json");
    }

    #[test]
    fn private_targets_are_rejected() {
        use std::net::IpAddr;
        for ip in [
            "127.0.0.1",
            "10.1.2.3",
            "192.168.1.10",
            "169.254.169.254",
            "100.64.0.1",
            "::1",
            "fd00::1",
        ] {
            assert!(is_private_ip(ip.parse::<IpAddr>().unwrap()), "{ip}");
        }
        for ip in ["8.8.8.8", "1.1.1.1", "2606:4700::1111"] {
            assert!(!is_private_ip(ip.parse::<IpAddr>().unwrap()), "{ip}");
        }
    }

    #[test]
    fn strip_common_root_keeps_mixed_roots() {
        let files = vec![
            ArchiveFile {
                path: "a/manifest.json".into(),
                bytes: vec![],
            },
            ArchiveFile {
                path: "b/main.mjs".into(),
                bytes: vec![],
            },
        ];
        let stripped = strip_common_root(files);
        assert_eq!(stripped[0].path, "a/manifest.json");
    }
}
