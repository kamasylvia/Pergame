//! Backup execution: snapshot collection, blob writing, and target fan-out.

use std::sync::Arc;

use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

use super::{
    BACKUP_VERSION, BackupManifest, BackupOptions, BackupStats, BackupStore, MAX_INLINE_ZIP_BYTES,
    ManifestEntry, ManifestFile, ResourceRow, SkippedFile, hex_sha, rfc3339_now,
};

/// Placeholder-path parity: Go fails fast when the storage client is nil.
pub async fn run_backup_no_storage(store: Arc<BackupStore>, user_id: Uuid, job_id: String) {
    let _ = user_id;
    store
        .update(&job_id, |j| {
            j.progress = 0;
            j.message = "minio client not initialized".into();
            j.error = Some("minio client not initialized".into());
        })
        .await;
}

/// Soft-link row projection for documents.json (spec note-links.md §2: the
/// backup carries the links table; a future restore rebuilds rows and skips +
/// reports ones pointing at missing nodes). UUIDs render as text to
/// keep the string-on-the-wire contract.
#[derive(Debug, sqlx::FromRow, Serialize)]
struct NodeLinkBackup {
    id: String,
    parent_id: String,
    node_id: String,
    created_timestamp: i64,
    order: i32,
}

/// Job phases (Go run() flow): collect → assemble zip → upload → presign.
/// `passphrase`: caller-supplied snapshot password (fork §2 semantics). None
/// = plain snapshot. The passphrase is never stored; unattended schedulers
/// pass the deployment fallback (`passphrase_from_env`), which is usually
/// unset — an unconfigured scheduler simply produces plain snapshots.
/// `zip_ttl_ms`: zero-target zip lifetime in the main bucket (user-chosen
/// 1h/1d/1w; unattended callers pass the 1h default).
#[allow(clippy::too_many_arguments)] // backup entrypoint: handles + ids + knobs, splitting buys nothing
pub async fn run_backup(
    store: Arc<BackupStore>,
    sql: sqlx::PgPool,
    storage: crate::storage::S3Storage,
    user_id: Uuid,
    job_id: String,
    options: BackupOptions,
    passphrase: Option<String>,
    zip_ttl_ms: i64,
) {
    macro_rules! progress {
        ($p:expr, $msg:expr) => {
            store
                .update(&job_id, |j| {
                    j.progress = $p;
                    j.message = $msg.into();
                })
                .await;
        };
    }
    macro_rules! fail {
        ($msg:expr) => {{
            let msg = $msg.to_string();
            store
                .update(&job_id, |j| {
                    j.progress = 0;
                    j.message = msg.clone();
                    j.error = Some(msg);
                })
                .await;
            return;
        }};
    }

    // Shared lock over this run's targets (fork §4): concurrent backups
    // allowed, but the same target never twice. The workspace is read-only
    // for the duration (released when the guard drops at function exit).
    // Targets resolve before locking: a collection-phase failure must not
    // hold anybody else's targets.
    let target_ids: Vec<Uuid> =
        match crate::target::repo::list_targets_for_user(&sql, user_id).await {
            Ok(t) => t.into_iter().map(|r| r.id).collect(),
            Err(_) => fail!("Failed to list backup targets"),
        };
    let _lock = match crate::locks::global().acquire_backup(&target_ids).await {
        Ok(g) => g,
        Err(crate::locks::LockError::TargetBusy(conflict)) => fail!(format!(
            "Backup target(s) busy: {}; retry when the running backup finishes",
            conflict
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )),
        Err(_) => fail!("Failed to acquire backup lock"),
    };
    if store.is_cancelled(&job_id) {
        return;
    }
    progress!(5, "Preparing backup...");

    let encryption = passphrase
        .as_ref()
        .filter(|p| !p.trim().is_empty())
        .map(|_| crate::snapcrypto::new_params());
    let passphrase = passphrase.filter(|p| !p.trim().is_empty());

    // ---------------- v2: collect members, then content-addressed blobs ----
    let mut members: Vec<(String, Vec<u8>)> = Vec::new();
    let mut files_meta: Vec<ManifestFile> = Vec::new();
    // (sha, e2e, bytes) for each attachment, uploaded in the write pass.
    let mut file_payloads: Vec<(String, bool, Vec<u8>)> = Vec::new();
    let mut stats = BackupStats::default();

    if options.include_documents {
        let rows: Vec<crate::models::NodeRow> = match sqlx::query_as::<_, crate::models::NodeRow>(
            "SELECT id, user_id, parent_id, name, description, tags, role, color, icon, thumbnail, \
             theme, accessibility, access, display, \"order\" AS node_order, content, content_compiled, size, \
             metadata, created_timestamp, updated_timestamp FROM nodes WHERE user_id = $1 \
             AND deleted_at IS NULL",
        )
        .bind(user_id)
        .fetch_all(&sql)
        .await
        {
            Ok(r) => r,
            Err(_) => fail!("Failed to prepare documents"),
        };
        stats.total_nodes = rows.len() as i64;
        stats.total_documents = rows.iter().filter(|r| r.role == 3).count() as i64;
        let documents: Vec<serde_json::Value> = rows
            .iter()
            .map(|r| {
                let mut v = serde_json::to_value(r).unwrap_or(serde_json::Value::Null);
                if !options.include_metadata
                    && let Some(obj) = v.as_object_mut()
                {
                    obj.remove("metadata");
                }
                v
            })
            .collect();

        // Soft links owned via the user's nodes (node ownership anchors the
        // row; parent must be one of the user's own folders by construction).
        let links: Vec<NodeLinkBackup> = match sqlx::query_as::<_, NodeLinkBackup>(
            "SELECT nl.id::text, nl.parent_id::text, nl.node_id::text, nl.created_timestamp, nl.\"order\" \
             FROM node_links nl JOIN nodes n ON n.id = nl.node_id \
             WHERE n.user_id = $1 AND n.deleted_at IS NULL ORDER BY nl.id",
        )
        .bind(user_id)
        .fetch_all(&sql)
        .await
        {
            Ok(r) => r,
            Err(_) => fail!("Failed to prepare document links"),
        };
        stats.total_links = links.len() as i64;
        progress!(40, "Packaging documents...");
        members.push((
            "documents.json".into(),
            serde_json::to_vec_pretty(&json!({
                "nodes": documents,
                "links": links,
            }))
            .unwrap_or_default(),
        ));
    }
    // ---------------- permissions + invitations (self-owned only) --------
    // Permissions store (node_id, grantee_username, permission): usernames,
    // not user ids — grantee ids would dangle cross-instance and waste
    // bytes. Restore resolves usernames back to local users, skipping
    // unknown ones. Invitations are self-contained rows (the code is the
    // identity), exported whole.
    if options.include_documents {
        #[derive(Debug, sqlx::FromRow, serde::Serialize)]
        struct PermissionBackup {
            node_id: String,
            grantee: String,
            permission: i32,
            created_timestamp: i64,
        }
        let permissions: Vec<PermissionBackup> = sqlx::query_as::<_, PermissionBackup>(
            "SELECT p.node_id::text AS node_id, u.username AS grantee, p.permission, \
             p.created_timestamp \
             FROM permissions p \
             JOIN nodes n ON n.id = p.node_id \
             JOIN users u ON u.id = p.user_id \
             WHERE n.user_id = $1 AND n.deleted_at IS NULL ORDER BY p.node_id",
        )
        .bind(user_id)
        .fetch_all(&sql)
        .await
        .unwrap_or_default();
        stats.total_permissions = permissions.len() as i64;
        members.push((
            "permissions.json".into(),
            serde_json::to_vec_pretty(&permissions).unwrap_or_default(),
        ));

        #[derive(Debug, sqlx::FromRow, serde::Serialize)]
        struct InvitationBackup {
            invitation_code: String,
            permission_level: i32,
            node_id: String,
            created_timestamp: i64,
        }
        let invitations: Vec<InvitationBackup> = sqlx::query_as::<_, InvitationBackup>(
            "SELECT ni.invitation_code, ni.permission_level, ni.node_id::text AS node_id, \
             ni.created_timestamp \
             FROM node_invitations ni JOIN nodes n ON n.id = ni.node_id \
             WHERE n.user_id = $1 AND n.deleted_at IS NULL ORDER BY ni.node_id",
        )
        .bind(user_id)
        .fetch_all(&sql)
        .await
        .unwrap_or_default();
        stats.total_invitations = invitations.len() as i64;
        members.push((
            "invitations.json".into(),
            serde_json::to_vec_pretty(&invitations).unwrap_or_default(),
        ));
    }
    if store.is_cancelled(&job_id) {
        return;
    }

    // ---------------- files (each attachment becomes a blob) ----------------
    if options.include_files {
        let resources: Vec<ResourceRow> = sqlx::query_as(
            r#"SELECT id, metadata, content, size FROM nodes
                   WHERE user_id = $1 AND role = 4 AND deleted_at IS NULL"#,
        )
        .bind(user_id)
        .fetch_all(&sql)
        .await
        .unwrap_or_default();
        stats.total_files = resources.len() as i64;
        progress!(60, "Archiving files...");
        for (res_id, metadata, original, size) in &resources {
            if store.is_cancelled(&job_id) {
                return;
            }
            let ext = original
                .as_deref()
                .and_then(|n| n.rsplit_once('.').map(|(_, e)| format!(".{e}")))
                .unwrap_or_default();
            let transformed = metadata
                .as_ref()
                .and_then(|m| m.get("transformed_path"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("{res_id}{ext}"));
            let sha_ptr = metadata
                .as_ref()
                .and_then(|m| m.get("content_sha256"))
                .and_then(|v| v.as_str())
                .filter(|s| s.len() == 64);
            let fetched = match sha_ptr {
                Some(sha) => storage.get_blob(sha).await,
                None => storage.get_user_object(user_id, &transformed).await,
            };
            let bytes = match fetched {
                Ok(b) => b,
                Err(_) => {
                    stats.skipped_files.push(SkippedFile {
                        id: res_id.to_string(),
                        name: original.clone().unwrap_or_default(),
                        reason: "object not found in storage".into(),
                    });
                    continue;
                }
            };
            let sha = hex_sha(&bytes);
            let (e2e, payload) = match (&encryption, &passphrase) {
                (Some(params), Some(pass)) => {
                    match crate::snapcrypto::encrypt(params, pass, &bytes) {
                        Ok(b) => (true, b),
                        Err(e) => {
                            fail!(format!("Failed to encrypt blob: {e}"));
                        }
                    }
                }
                _ => (false, bytes.clone()),
            };
            // Held in memory: direct-write uploads everything to the user's
            // targets in one pass below (dedup happens per target there).
            file_payloads.push((sha.clone(), e2e, payload));
            files_meta.push(ManifestFile {
                id: res_id.to_string(),
                key: transformed,
                original_path: original.clone(),
                sha256: sha,
                size: (*size).unwrap_or(bytes.len() as i64) as u64,
            });
        }
    }

    // ---------------- user_settings.json ----------------
    if options.include_settings
        && let Some((g, e, a)) =
            sqlx::query_as::<_, (serde_json::Value, serde_json::Value, serde_json::Value)>(
                "SELECT general, editor, advanced FROM user_settings WHERE user_id = $1",
            )
            .bind(user_id)
            .fetch_optional(&sql)
            .await
            .ok()
            .flatten()
    {
        let settings = serde_json::json!({
            "user_id": user_id.to_string(),
            "general": g, "editor": e, "advanced": a,
        });
        members.push((
            "user_settings.json".into(),
            serde_json::to_vec_pretty(&settings).unwrap_or_default(),
        ));
    }

    // ---------------- trash.json (soft-deleted nodes, merge on restore) --
    // Live documents.json stays live-only; trashed rows ride separately so
    // restore can replace live data while merging trash (never clobbering
    // a node the user already recovered).
    if options.include_documents {
        #[derive(Debug, sqlx::FromRow, serde::Serialize)]
        struct TrashNodeBackup {
            id: Uuid,
            user_id: Uuid,
            parent_id: Option<Uuid>,
            name: String,
            description: Option<String>,
            tags: Option<String>,
            role: i32,
            color: Option<i32>,
            icon: Option<String>,
            thumbnail: Option<String>,
            theme: Option<String>,
            accessibility: Option<i32>,
            access: i32,
            display: Option<i32>,
            #[serde(rename = "order")]
            #[sqlx(rename = "node_order")]
            node_order: Option<i32>,
            content: Option<String>,
            content_compiled: Option<String>,
            size: Option<i64>,
            metadata: Option<serde_json::Value>,
            created_timestamp: i64,
            updated_timestamp: i64,
            deleted_at: i64,
        }
        let trashed: Vec<TrashNodeBackup> = sqlx::query_as::<_, TrashNodeBackup>(
            "SELECT id, user_id, parent_id, name, description, tags, role, color, icon, thumbnail, \
             theme, accessibility, access, display, \"order\" AS node_order, content, content_compiled, size, \
             metadata, created_timestamp, updated_timestamp, deleted_at FROM nodes WHERE user_id = $1 \
             AND deleted_at IS NOT NULL",
        )
        .bind(user_id)
        .fetch_all(&sql)
        .await
        .unwrap_or_default();
        stats.total_trashed = trashed.len() as i64;
        if !trashed.is_empty() {
            members.push((
                "trash.json".into(),
                serde_json::to_vec_pretty(&json!({ "nodes": trashed })).unwrap_or_default(),
            ));
        }
    }

    // ---------------- profile.json (display fields only) ------------------
    // No credentials, no role/type: restoring a profile must never change
    // who you are or what you can do — only how your name looks.
    if options.include_settings {
        #[derive(Debug, sqlx::FromRow, serde::Serialize)]
        struct ProfileBackup {
            username: String,
            firstname: Option<String>,
            lastname: Option<String>,
            avatar: Option<String>,
            email: Option<String>,
            created_timestamp: i64,
        }
        if let Ok(Some(profile)) = sqlx::query_as::<_, ProfileBackup>(
            "SELECT username, firstname, lastname, avatar, email, created_timestamp \
                 FROM users WHERE id = $1",
        )
        .bind(user_id)
        .fetch_optional(&sql)
        .await
        {
            members.push((
                "profile.json".into(),
                serde_json::to_vec_pretty(&profile).unwrap_or_default(),
            ));
        }
    }
    let snapshot_id = crate::auth::service::now_ms();
    let manifest = BackupManifest {
        version: BACKUP_VERSION.into(),
        id: snapshot_id.to_string(),
        created_at: rfc3339_now(),
        user_id: user_id.to_string(),
        memo: options.memo.clone(),
        encryption: encryption.clone(),
        options: options.clone(),
        statistics: stats.clone(),
        entries: members
            .iter()
            .map(|(path, bytes)| ManifestEntry {
                path: path.clone(),
                sha256: hex_sha(bytes),
                size: bytes.len() as u64,
            })
            .collect(),
        files: files_meta.clone(),
        logical_bytes: members.iter().map(|(_, b)| b.len() as u64).sum::<u64>()
            + files_meta.iter().map(|f| f.size).sum::<u64>(),
    };
    progress!(85, "Writing blobs...");

    // Encrypt member blobs once; every target receives identical bytes.
    let mut member_payloads: Vec<(String, bool, Vec<u8>)> = Vec::with_capacity(members.len());
    for (_path, bytes) in &members {
        let sha = hex_sha(bytes);
        let (e2e, payload) = match (&encryption, &passphrase) {
            (Some(params), Some(pass)) => match crate::snapcrypto::encrypt(params, pass, bytes) {
                Ok(b) => (true, b),
                Err(e) => {
                    fail!(format!("Failed to encrypt blob: {e}"));
                }
            },
            _ => (false, bytes.clone()),
        };
        member_payloads.push((sha, e2e, payload));
    }

    // Direct-write: the snapshot lands on the user's own targets, never on
    // the service bucket. Zero targets = fork §4 "save zip locally": the
    // portable set is held in memory for one download, nothing is persisted.
    // (Target ids come from the lock-time query above — same list we hold.)
    if target_ids.is_empty() {
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).unwrap_or_default();
        match crate::restore::build_portable_zip_in_memory(
            &manifest,
            &manifest_bytes,
            &member_payloads,
            &file_payloads,
        ) {
            Ok(zip) if zip.len() <= MAX_INLINE_ZIP_BYTES => {
                let (expires_ms, token) = store
                    .put_inline_zip(&storage, user_id, &job_id, zip, zip_ttl_ms)
                    .await;
                store
                    .update(&job_id, |j| {
                        j.progress = 100;
                        j.message =
                            "Snapshot packaged for download (no targets configured).".into();
                        j.download_url =
                            Some(format!("/api/backup/{job_id}/download?token={token}"));
                        j.expires_at = Some(expires_ms.to_string());
                    })
                    .await;
                return;
            }
            Ok(zip) => fail!(format!(
                "Snapshot is {} MB with no targets configured (limit {} MB); configure a backup target first",
                zip.len() / 1_048_576,
                MAX_INLINE_ZIP_BYTES / 1_048_576
            )),
            Err(e) => fail!(format!("Failed to package snapshot: {e}")),
        }
    }
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).unwrap_or_default();
    let results = crate::target::fanout::write_snapshot_to_targets(
        &sql,
        user_id,
        &manifest,
        &manifest_bytes,
        &member_payloads,
        &file_payloads,
    )
    .await;
    let mut failed = 0usize;
    for r in &results {
        if let Some(e) = &r.error {
            failed += 1;
            tracing::warn!(target = %r.target, error = %e, "direct-write failed");
        } else {
            tracing::info!(target = %r.target, copied = r.copied, skipped = r.skipped, "direct-write done");
        }
    }
    if failed == results.len() && !results.is_empty() {
        fail!(format!(
            "All {} target(s) failed: {}",
            results.len(),
            results
                .iter()
                .filter_map(|r| r.error.clone())
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    let ok = results.len() - failed;
    store
        .update(&job_id, |j| {
            j.progress = 100;
            j.message = if failed == 0 {
                format!("Snapshot completed successfully ({} target(s)).", ok)
            } else {
                format!("Snapshot completed with {failed} failed target(s) ({ok} ok).")
            };
            // download_url intentionally unset: snapshots download through
            // the authenticated /backup/snapshots/:id/download endpoint.
        })
        .await;
}
