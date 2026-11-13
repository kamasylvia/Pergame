//! Restore dry-run preview (`plan_from_zip`) and v2 snapshot assembly.

use serde::Serialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use std::io::Read as _;
use uuid::Uuid;
use zip::ZipArchive;

use crate::error::ApiError;

use super::{MAX_ENTRIES, MAX_ENTRY_BYTES, as_opt_uuid, as_string, as_uuid};

/// Dry-run plan for a restore (spec backup-restore.md: conflict/collision
/// preview without writing). Counts are computed against the caller's
/// current data; file collisions compare incoming transformed paths with
/// paths already referenced by the caller's resource nodes.
#[derive(Debug, Default, Serialize)]
pub struct RestorePlan {
    pub format_version: String,
    pub incoming_nodes: usize,
    pub current_nodes: usize,
    pub nodes_replaced: usize, // incoming ids already present
    pub nodes_added: usize,
    pub nodes_removed: usize,  // current ids absent from the archive
    pub nodes_orphaned: usize, // incoming nodes whose parent is missing
    pub incoming_links: usize,
    pub links_unresolvable: usize,
    pub incoming_files: usize,
    pub files_colliding: usize,
    pub settings_in_archive: bool,
    pub incoming_permissions: usize,
    pub permissions_unresolvable: usize, // node missing or grantee unknown
    pub incoming_invitations: usize,
    pub invitations_conflicting: usize, // code already exists
    pub incoming_trashed: usize,
    pub trashed_present: usize, // ids already in the live table (merge skips)
    pub profile_in_archive: bool,
    pub warnings: Vec<String>,
}
/// Dry-run a restore: counts/phantom collisions without writing.
pub async fn plan_from_zip(
    sql: PgPool,
    user_id: Uuid,
    bytes: Vec<u8>,
) -> Result<RestorePlan, ApiError> {
    let mut plan = RestorePlan::default();
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|e| ApiError::BadRequest(format!("invalid zip archive: {e}")))?;
    if archive.len() > MAX_ENTRIES {
        return Err(ApiError::BadRequest("archive has too many entries".into()));
    }

    let mut read_member = |name: &str| -> Option<String> {
        let mut f = archive.by_name(name).ok()?;
        if f.size() > MAX_ENTRY_BYTES {
            return None;
        }
        let mut s = String::new();
        f.read_to_string(&mut s).ok()?;
        Some(s)
    };

    let manifest: Value = serde_json::from_str(
        &read_member("manifest.json")
            .ok_or(ApiError::BadRequest("manifest.json missing".into()))?,
    )
    .map_err(|e| ApiError::BadRequest(format!("manifest.json: {e}")))?;
    plan.format_version = manifest
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("?")
        .to_string();

    let docs: Value = serde_json::from_str(
        &read_member("documents.json")
            .ok_or(ApiError::BadRequest("documents.json missing".into()))?,
    )
    .map_err(|e| ApiError::BadRequest(format!("documents.json: {e}")))?;
    let (nodes, links) = match &docs {
        Value::Array(arr) => {
            plan.warnings
                .push("archive is v1.1.0 (bare node array): links absent".into());
            (arr.clone(), Vec::new())
        }
        Value::Object(obj) => (
            obj.get("nodes")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default(),
            obj.get("links")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default(),
        ),
        _ => return Err(ApiError::BadRequest("documents.json shape invalid".into())),
    };

    plan.incoming_nodes = nodes.len();
    plan.incoming_links = links.len();

    // New members: permissions / invitations / trash (absent in old archives).
    let permissions: Vec<Value> = read_member("permissions.json")
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let invitations: Vec<Value> = read_member("invitations.json")
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let trash_nodes: Vec<Value> = read_member("trash.json")
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| v.get("nodes").and_then(|n| n.as_array()).cloned())
        .unwrap_or_default();
    let settings_present = read_member("user_settings.json").is_some();
    let profile_present = read_member("profile.json").is_some();
    plan.settings_in_archive = settings_present;
    plan.profile_in_archive = profile_present;
    plan.incoming_permissions = permissions.len();
    plan.incoming_invitations = invitations.len();
    plan.incoming_trashed = trash_nodes.len();
    let mut incoming_ids = std::collections::HashSet::new();
    let mut incoming_parents_missing = 0usize;
    for n in &nodes {
        if let Some(id) = as_uuid(n.get("id")) {
            incoming_ids.insert(id);
        }
    }
    for n in &nodes {
        let parent = as_opt_uuid(n.get("parent_id"));
        if let Some(p) = parent
            && !incoming_ids.contains(&p)
        {
            incoming_parents_missing += 1;
        }
    }
    if incoming_parents_missing > 0 {
        plan.warnings.push(format!(
            "{incoming_parents_missing} node(s) reference parents outside the archive and will be promoted to top level"
        ));
    }
    plan.nodes_orphaned = incoming_parents_missing;

    for l in &links {
        let (Some(node_id), Some(parent_id)) =
            (as_uuid(l.get("node_id")), as_uuid(l.get("parent_id")))
        else {
            plan.links_unresolvable += 1;
            continue;
        };
        if !incoming_ids.contains(&node_id) || !incoming_ids.contains(&parent_id) {
            plan.links_unresolvable += 1;
        }
    }

    // Current state.
    let (current_nodes,): (i64,) = sqlx::query_as("SELECT count(*) FROM nodes WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&sql)
        .await
        .map_err(ApiError::from)
        .map_err(|e| ApiError::Internal(Box::new(std::io::Error::other(e.to_string()))))?;
    plan.current_nodes = current_nodes as usize;

    let (current_ids,): (Vec<Uuid>,) =
        sqlx::query_as("SELECT COALESCE(array_agg(id), '{}') FROM nodes WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&sql)
            .await
            .map_err(ApiError::from)
            .map_err(|e| ApiError::Internal(Box::new(std::io::Error::other(e.to_string()))))?;
    let current_set: std::collections::HashSet<Uuid> = current_ids.into_iter().collect();
    plan.nodes_replaced = incoming_ids
        .iter()
        .filter(|id| current_set.contains(id))
        .count();
    plan.nodes_added = incoming_ids
        .iter()
        .filter(|id| !current_set.contains(id))
        .count();
    plan.nodes_removed = current_set
        .iter()
        .filter(|id| !incoming_ids.contains(id))
        .count();

    // files: transformed paths currently referenced by the caller's resources
    if let Ok(mut idx_file) = archive.by_name("files_index.json") {
        let mut s = String::new();
        if idx_file.size() <= MAX_ENTRY_BYTES
            && idx_file.read_to_string(&mut s).is_ok()
            && let Ok(entries) = serde_json::from_str::<Vec<Value>>(&s)
        {
            plan.incoming_files = entries.len();
            let (existing,): (Vec<String>,) = sqlx::query_as(
                "SELECT COALESCE(array_agg(metadata->>'transformed_path'), '{}') FROM nodes \
                 WHERE user_id = $1 AND role = 4 AND metadata ? 'transformed_path'",
            )
            .bind(user_id)
            .fetch_one(&sql)
            .await
            .map_err(ApiError::from)
            .map_err(|e| ApiError::Internal(Box::new(std::io::Error::other(e.to_string()))))?;
            let existing: std::collections::HashSet<String> = existing.into_iter().collect();
            // A file collides when its restored key (transformed_path of the
            // matching incoming node, else <id><ext>) is already in the store.
            let incoming_by_id: std::collections::HashMap<Uuid, &Value> = nodes
                .iter()
                .filter_map(|n| as_uuid(n.get("id")).map(|i| (i, n)))
                .collect();
            for e in &entries {
                let Some(id) = as_uuid(e.get("id")) else {
                    continue;
                };
                let ext = as_string(e.get("path"))
                    .and_then(|p| p.rsplit_once('.').map(|(_, e)| format!(".{e}")))
                    .unwrap_or_default();
                let key = incoming_by_id
                    .get(&id)
                    .and_then(|n| {
                        n.get("metadata")
                            .and_then(|m| m.get("transformed_path"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_else(|| format!("{id}{ext}"));
                if existing.contains(&key) {
                    plan.files_colliding += 1;
                }
            }
        }
    }

    // Permissions: node must be incoming, grantee must exist.
    {
        let (usernames,): (Vec<String>,) =
            sqlx::query_as("SELECT COALESCE(array_agg(username), '{}') FROM users")
                .fetch_one(&sql)
                .await
                .unwrap_or((Vec::new(),));
        let known: std::collections::HashSet<&str> = usernames.iter().map(|s| s.as_str()).collect();
        for p in &permissions {
            let node_ok = as_uuid(p.get("node_id")).is_some_and(|id| incoming_ids.contains(&id));
            let user_ok = p
                .get("grantee")
                .and_then(|v| v.as_str())
                .is_some_and(|g| known.contains(g));
            if !(node_ok && user_ok) {
                plan.permissions_unresolvable += 1;
            }
        }
    }

    // Invitations: codes already present will be skipped on restore.
    {
        let (codes,): (Vec<String>,) = sqlx::query_as(
            "SELECT COALESCE(array_agg(invitation_code), '{}') FROM node_invitations",
        )
        .fetch_one(&sql)
        .await
        .unwrap_or((Vec::new(),));
        let existing: std::collections::HashSet<&str> = codes.iter().map(|s| s.as_str()).collect();
        for inv in &invitations {
            let node_ok = as_uuid(inv.get("node_id")).is_some_and(|id| incoming_ids.contains(&id));
            let fresh = inv
                .get("invitation_code")
                .and_then(|v| v.as_str())
                .is_some_and(|c| !existing.contains(c));
            if !(node_ok && fresh) {
                plan.invitations_conflicting += 1;
            }
        }
    }

    // Trash: ids already live are merge-skipped (user recovered them).
    for n in &trash_nodes {
        if let Some(id) = as_uuid(n.get("id"))
            && current_set.contains(&id)
        {
            plan.trashed_present += 1;
        }
    }

    Ok(plan)
}

/// Assembles a v2 snapshot (manifest + deduped blobs) back into the v1 zip
/// shape the restore/preview engine consumes, decrypting blobs when the
/// manifest says the snapshot is encrypted. Reads from a connected target
/// (direct-write read path).
pub async fn assemble_snapshot_zip(
    conn: &crate::target::config::TargetConn,
    manifest: crate::backup::BackupManifest,
    passphrase: Option<&str>,
) -> Result<Vec<u8>, ApiError> {
    let passphrase = passphrase
        .filter(|p| !p.trim().is_empty())
        .map(|s| s.to_string());
    if manifest.encryption.is_some() && passphrase.is_none() {
        return Err(ApiError::BadRequest(
            "snapshot is encrypted but no passphrase was supplied".into(),
        ));
    }
    let e2e = manifest.encryption.is_some();
    let dir = if e2e { "blobs-e2e" } else { "blobs" };
    let uid = &manifest.user_id;
    let mut archive = crate::backup::SimpleArchive::new();
    // The manifest itself is an archive member (the engine gates on it).
    archive.add(
        "manifest.json",
        &serde_json::to_vec_pretty(&manifest).unwrap_or_default(),
    );
    for e in &manifest.entries {
        let blob = conn.get(&format!("{uid}/{dir}/{}", e.sha256)).await?;
        let plain = match (&manifest.encryption, &passphrase) {
            (Some(params), Some(pass)) => crate::snapcrypto::decrypt(params, pass, &blob)
                .map_err(|e| ApiError::BadRequest(format!("snapshot decrypt: {e}")))?,
            _ => blob,
        };
        archive.add(&e.path, &plain);
    }
    // files_index.json rebuilt from manifest.files; zip member paths follow the
    // v1 layout (files/<id><ext>).
    let mut idx = Vec::with_capacity(manifest.files.len());
    for f in &manifest.files {
        let ext = f
            .key
            .rsplit_once('.')
            .map(|(_, e)| format!(".{e}"))
            .unwrap_or_default();
        let path = format!("files/{}{}", f.id, ext);
        idx.push(json!({
            "id": f.id,
            "path": path,
            "original_path": f.original_path,
            "size": f.size,
        }));
    }
    archive.add(
        "files_index.json",
        &serde_json::to_vec_pretty(&idx).unwrap_or_default(),
    );
    for f in &manifest.files {
        let ext = f
            .key
            .rsplit_once('.')
            .map(|(_, e)| format!(".{e}"))
            .unwrap_or_default();
        let path = format!("files/{}{}", f.id, ext);
        let blob = conn.get(&format!("{uid}/{dir}/{}", f.sha256)).await?;
        let plain = match (&manifest.encryption, &passphrase) {
            (Some(params), Some(pass)) => crate::snapcrypto::decrypt(params, pass, &blob)
                .map_err(|e| ApiError::BadRequest(format!("snapshot decrypt: {e}")))?,
            _ => blob,
        };
        archive.add_stored(path, &plain);
    }
    archive
        .finish()
        .map_err(|e| ApiError::BadRequest(format!("snapshot assembly failed: {e}")))
}
