//! Restore apply: replace-strategy engine (`restore_from_zip`).

use super::{
    MAX_ENTRIES, MAX_ENTRY_BYTES, MAX_FORMAT, RestoreReport, as_i64, as_opt_i64, as_opt_uuid,
    as_string, as_uuid,
    members::{restore_invitations, restore_permissions},
};
use crate::error::ApiError;
use serde_json::Value;
use sqlx::PgPool;
use std::io::Read as _;
use uuid::Uuid;
use zip::ZipArchive;

/// Replace the caller's data from a v1-shaped zip (force gates non-empty workspaces).
pub async fn restore_from_zip(
    sql: PgPool,
    storage: Option<crate::storage::S3Storage>,
    user_id: Uuid,
    bytes: Vec<u8>,
    force: bool,
) -> Result<RestoreReport, ApiError> {
    // Fork §5 parity: refuse to overwrite a non-empty workspace unless the
    // caller explicitly forces it (after seeing the dry-run preview). Trash
    // does not count — restoring over trash is always allowed.
    if !force {
        let (live,): (i64,) =
            sqlx::query_as("SELECT count(*) FROM nodes WHERE user_id = $1 AND deleted_at IS NULL")
                .bind(user_id)
                .fetch_one(&sql)
                .await
                .unwrap_or((0,));
        if live > 0 {
            return Err(ApiError::BadRequest(format!(
                "workspace has {live} live node(s); preview the restore, then retry with force to overwrite"
            )));
        }
    }
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|e| ApiError::BadRequest(format!("invalid zip archive: {e}")))?;
    if archive.len() > MAX_ENTRIES {
        return Err(ApiError::BadRequest("archive has too many entries".into()));
    }

    // ---- manifest.json (format gate) ----
    let manifest: Value = {
        let mut f = archive
            .by_name("manifest.json")
            .map_err(|_| ApiError::BadRequest("manifest.json missing".into()))?;
        let mut s = String::new();
        if f.size() > MAX_ENTRY_BYTES {
            return Err(ApiError::BadRequest("manifest.json too large".into()));
        }
        f.read_to_string(&mut s)
            .map_err(|_| ApiError::BadRequest("manifest.json unreadable".into()))?;
        serde_json::from_str(&s).map_err(|e| ApiError::BadRequest(format!("manifest.json: {e}")))?
    };
    let version = manifest
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("0.0.0")
        .to_string();
    let (maj, min): (u32, u32) = {
        let parts: Vec<_> = version.split('.').take(2).collect();
        match (parts.first(), parts.get(1)) {
            (Some(m), Some(n)) => (
                m.parse().unwrap_or(0),
                n.trim_end_matches(|c: char| !c.is_ascii_digit())
                    .parse()
                    .unwrap_or(0),
            ),
            _ => (0, 0),
        }
    };
    if maj > MAX_FORMAT.0 || (maj == MAX_FORMAT.0 && min > MAX_FORMAT.1) {
        return Err(ApiError::BadRequest(format!(
            "backup format {version} is newer than supported {}.{}",
            MAX_FORMAT.0, MAX_FORMAT.1
        )));
    }

    // ---- documents.json (1.1.0 bare array or 1.2.0 {nodes, links}) ----
    let docs: Value = {
        let mut f = archive
            .by_name("documents.json")
            .map_err(|_| ApiError::BadRequest("documents.json missing".into()))?;
        if f.size() > MAX_ENTRY_BYTES {
            return Err(ApiError::BadRequest("documents.json too large".into()));
        }
        let mut s = String::new();
        f.read_to_string(&mut s)
            .map_err(|_| ApiError::BadRequest("documents.json unreadable".into()))?;
        serde_json::from_str(&s)
            .map_err(|e| ApiError::BadRequest(format!("documents.json: {e}")))?
    };
    let (nodes_raw, links_raw, mut warnings) = match &docs {
        Value::Array(arr) => (
            arr.clone(),
            Vec::new(),
            vec!["archive is v1.1.0 (bare node array): links absent".to_string()],
        ),
        Value::Object(obj) => (
            obj.get("nodes")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default(),
            obj.get("links")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default(),
            Vec::new(),
        ),
        _ => return Err(ApiError::BadRequest("documents.json shape invalid".into())),
    };

    // ---- optional members (absent in older archives: skip silently) ----
    let read_member =
        |archive: &mut ZipArchive<std::io::Cursor<Vec<u8>>>, name: &str| -> Vec<Value> {
            let mut f = match archive.by_name(name) {
                Ok(f) => f,
                Err(_) => return Vec::new(),
            };
            if f.size() > MAX_ENTRY_BYTES {
                return Vec::new();
            }
            let mut s = String::new();
            if f.read_to_string(&mut s).is_err() {
                return Vec::new();
            }
            match serde_json::from_str::<Value>(&s) {
                Ok(Value::Array(arr)) => arr,
                Ok(Value::Object(obj)) if name == "trash.json" => obj
                    .get("nodes")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default(),
                _ => Vec::new(),
            }
        };
    let permissions_raw = read_member(&mut archive, "permissions.json");
    let invitations_raw = read_member(&mut archive, "invitations.json");
    let trash_raw = read_member(&mut archive, "trash.json");
    let profile_raw: Option<Value> = (|| {
        let mut f = archive.by_name("profile.json").ok()?;
        if f.size() > MAX_ENTRY_BYTES {
            return None;
        }
        let mut s = String::new();
        f.read_to_string(&mut s).ok()?;
        serde_json::from_str(&s).ok()
    })();

    let mut report = RestoreReport::default();
    report.warnings.append(&mut warnings);

    // ---- transactional replace: drop current nodes, rebuild ----
    let mut tx = sql
        .begin()
        .await
        .map_err(ApiError::from)
        .map_err(|e| ApiError::Internal(Box::new(std::io::Error::other(e.to_string()))))?;
    sqlx::query("DELETE FROM nodes WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::from)
        .map_err(|e| ApiError::Internal(Box::new(std::io::Error::other(e.to_string()))))?;

    // First pass: collect ids so parent references can be validated.
    let mut node_ids = std::collections::HashSet::new();
    for n in &nodes_raw {
        if let Some(id) = as_uuid(n.get("id")) {
            node_ids.insert(id);
        }
    }

    // Real insert: description/tags/text columns are plain strings or NULL.
    macro_rules! opt_str {
        ($n:expr, $k:expr) => {
            match $n.get($k) {
                Some(Value::Null) | None => None,
                Some(v) => v.as_str().map(|s| s.to_string()),
            }
        };
    }

    let mut restored_parent_fixed = 0usize;
    // Archive order is arbitrary: a child may precede its parent, and the
    // nodes_parent_id_fkey would reject the insert. Insert parent-less first,
    // then wire parents in a second pass (all ids exist by then).
    let mut deferred_parents: Vec<(Uuid, Uuid)> = Vec::new();
    for n in &nodes_raw {
        let Some(id) = as_uuid(n.get("id")) else {
            report.warnings.push("node without id skipped".into());
            continue;
        };
        // Parent must be present in the archive; orphan the node otherwise.
        // Deferred to a second pass (see above): the archive may list a
        // child before its parent.
        let parent = as_opt_uuid(n.get("parent_id"));
        match parent {
            Some(p) if node_ids.contains(&p) => deferred_parents.push((id, p)),
            Some(p) => {
                report.warnings.push(format!(
                    "node {id}: parent {p} not in archive, promoted to top level"
                ));
                restored_parent_fixed += 1;
            }
            None => {}
        };
        let metadata = n.get("metadata").cloned().unwrap_or(Value::Null);
        sqlx::query(
            r#"INSERT INTO nodes (id, user_id, parent_id, name, description, tags, role, color,
                   icon, thumbnail, theme, accessibility, access, display, "order",
                   content, content_compiled, size, metadata, created_timestamp, updated_timestamp)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,
                       $19::jsonb,$20,$21)"#,
        )
        .bind(id)
        .bind(user_id)
        .bind(Option::<Uuid>::None)
        .bind(as_string(n.get("name")).unwrap_or_default())
        .bind(opt_str!(n, "description"))
        .bind(opt_str!(n, "tags"))
        .bind(as_i64(n.get("role")).unwrap_or(3) as i32)
        .bind(as_opt_i64(n.get("color")).map(|v| v as i32))
        .bind(opt_str!(n, "icon"))
        .bind(opt_str!(n, "thumbnail"))
        .bind(opt_str!(n, "theme"))
        .bind(as_opt_i64(n.get("accessibility")).map(|v| v as i32))
        .bind(as_i64(n.get("access")).unwrap_or(0) as i32)
        .bind(as_opt_i64(n.get("display")).map(|v| v as i32))
        .bind(as_opt_i64(n.get("order")).map(|v| v as i32))
        .bind(opt_str!(n, "content"))
        .bind(opt_str!(n, "content_compiled"))
        .bind(as_opt_i64(n.get("size")))
        .bind(&metadata)
        .bind(as_i64(n.get("created_timestamp")).unwrap_or(0))
        .bind(as_i64(n.get("updated_timestamp")).unwrap_or(0))
        .execute(&mut *tx)
        .await
        .map_err(ApiError::from)
        .map_err(|e| ApiError::Internal(Box::new(std::io::Error::other(e.to_string()))))?;
        report.nodes += 1;
    }

    // Second pass: wire parents now that every id exists.
    for (id, parent) in &deferred_parents {
        sqlx::query("UPDATE nodes SET parent_id = $1 WHERE id = $2")
            .bind(parent)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(ApiError::from)
            .map_err(|e| ApiError::Internal(Box::new(std::io::Error::other(e.to_string()))))?;
    }
    // ---- links: only rows whose node AND parent both live in the archive ----
    for l in &links_raw {
        let Some(id) = as_uuid(l.get("id")) else {
            continue;
        };
        let (Some(node_id), Some(parent_id)) =
            (as_uuid(l.get("node_id")), as_uuid(l.get("parent_id")))
        else {
            report.links_skipped.push(format!("{id}:missing fields"));
            continue;
        };
        if !node_ids.contains(&node_id) || !node_ids.contains(&parent_id) {
            report.links_skipped.push(format!("{id}:missing node"));
            continue;
        }
        sqlx::query(
            r#"INSERT INTO node_links (id, parent_id, node_id, "order", created_timestamp)
               VALUES ($1,$2,$3,$4,$5)"#,
        )
        .bind(id)
        .bind(parent_id)
        .bind(node_id)
        .bind(as_i64(l.get("order")).unwrap_or(0) as i32)
        .bind(as_i64(l.get("created_timestamp")).unwrap_or(0))
        .execute(&mut *tx)
        .await
        .map_err(ApiError::from)
        .map_err(|e| ApiError::Internal(Box::new(std::io::Error::other(e.to_string()))))?;
        report.links += 1;
    }

    restore_permissions(&mut tx, &permissions_raw, &node_ids, &mut report).await?;
    restore_invitations(&mut tx, &invitations_raw, &node_ids, &mut report).await?;

    // ---- trash: merge, never replace. A trashed row is only inserted when
    // its id is absent — the user may have recovered it since the snapshot.
    for n in &trash_raw {
        let Some(id) = as_uuid(n.get("id")) else {
            report.trashed_skipped.push("missing id".into());
            continue;
        };
        let inserted = sqlx::query(
            r#"INSERT INTO nodes (id, user_id, parent_id, name, description, tags, role, color,
                   icon, thumbnail, theme, accessibility, access, display, "order",
                   content, content_compiled, size, metadata, created_timestamp, updated_timestamp,
                   deleted_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,
                       $19::jsonb,$20,$21,$22)
               ON CONFLICT (id) DO NOTHING"#,
        )
        .bind(id)
        .bind(user_id)
        .bind(as_opt_uuid(n.get("parent_id")))
        .bind(as_string(n.get("name")).unwrap_or_default())
        .bind(opt_str!(n, "description"))
        .bind(opt_str!(n, "tags"))
        .bind(as_i64(n.get("role")).unwrap_or(3) as i32)
        .bind(as_opt_i64(n.get("color")).map(|v| v as i32))
        .bind(opt_str!(n, "icon"))
        .bind(opt_str!(n, "thumbnail"))
        .bind(opt_str!(n, "theme"))
        .bind(as_opt_i64(n.get("accessibility")).map(|v| v as i32))
        .bind(as_i64(n.get("access")).unwrap_or(0) as i32)
        .bind(as_opt_i64(n.get("display")).map(|v| v as i32))
        .bind(as_opt_i64(n.get("order")).map(|v| v as i32))
        .bind(opt_str!(n, "content"))
        .bind(opt_str!(n, "content_compiled"))
        .bind(as_opt_i64(n.get("size")))
        .bind(n.get("metadata").cloned().unwrap_or(Value::Null))
        .bind(as_i64(n.get("created_timestamp")).unwrap_or(0))
        .bind(as_i64(n.get("updated_timestamp")).unwrap_or(0))
        .bind(as_i64(n.get("deleted_at")).unwrap_or(0))
        .execute(&mut *tx)
        .await
        .map(|r| r.rows_affected())
        .unwrap_or(0);
        if inserted == 0 {
            report.trashed_skipped.push(format!("{id}:already present"));
        } else {
            report.trashed += 1;
        }
    }

    tx.commit()
        .await
        .map_err(ApiError::from)
        .map_err(|e| ApiError::Internal(Box::new(std::io::Error::other(e.to_string()))))?;
    if restored_parent_fixed > 0 {
        report.warnings.push(format!(
            "{restored_parent_fixed} node(s) re-parented to top level"
        ));
    }

    // ---- user_settings.json (separate archive member) ----
    if let Ok(mut f) = archive.by_name("user_settings.json") {
        let mut s = String::new();
        if f.size() <= MAX_ENTRY_BYTES
            && f.read_to_string(&mut s).is_ok()
            && let Ok(st) = serde_json::from_str::<Value>(&s)
        {
            let general = st.get("general").cloned().unwrap_or(Value::Null);
            let editor = st.get("editor").cloned().unwrap_or(Value::Null);
            let advanced = st.get("advanced").cloned().unwrap_or(Value::Null);
            if sqlx::query(
                "INSERT INTO user_settings (user_id, general, editor, advanced) \
                 VALUES ($1,$2::jsonb,$3::jsonb,$4::jsonb) \
                 ON CONFLICT (user_id) DO UPDATE SET \
                 general = EXCLUDED.general, editor = EXCLUDED.editor, advanced = EXCLUDED.advanced",
            )
            .bind(user_id)
            .bind(&general)
            .bind(&editor)
            .bind(&advanced)
            .execute(&sql)
            .await
            .is_ok()
            {
                report.settings = true;
            }
        }
    }

    // ---- profile.json: display fields only. username/email apply only when
    // they collide with nobody else's row; role/type/password are never
    // touched (restoring a profile must not change identity or powers).
    if let Some(profile) = &profile_raw {
        let username = as_string(profile.get("username")).unwrap_or_default();
        let email = match profile.get("email") {
            Some(Value::Null) | None => None,
            Some(v) => v.as_str().map(|s| s.to_string()),
        };
        // Gate each unique field on non-collision; NULL email always applies.
        let username_ok = if username.is_empty() {
            false
        } else {
            let (taken,): (bool,) = sqlx::query_as(
                "SELECT EXISTS(SELECT 1 FROM users WHERE username = $1 AND id <> $2)",
            )
            .bind(&username)
            .bind(user_id)
            .fetch_one(&sql)
            .await
            .unwrap_or((true,));
            !taken
        };
        if !username.is_empty() && !username_ok {
            report.warnings.push(format!(
                "profile: username '{username}' taken, kept current"
            ));
        }
        let email_ok = match &email {
            None => true,
            Some(e) => {
                let (taken,): (bool,) = sqlx::query_as(
                    "SELECT EXISTS(SELECT 1 FROM users WHERE email = $1 AND id <> $2)",
                )
                .bind(e)
                .bind(user_id)
                .fetch_one(&sql)
                .await
                .unwrap_or((true,));
                !taken
            }
        };
        if let Some(e) = &email
            && !email_ok
        {
            report
                .warnings
                .push(format!("profile: email '{e}' taken, kept current"));
        }
        let applied = sqlx::query(
            "UPDATE users SET username = COALESCE($2, username), firstname = COALESCE($3, firstname), \
             lastname = COALESCE($4, lastname), avatar = COALESCE($5, avatar), email = COALESCE($6, email), \
             updated_timestamp = $7 WHERE id = $1",
        )
        .bind(user_id)
        .bind(if username_ok { Some(username) } else { None::<String> })
        .bind(opt_str!(profile, "firstname"))
        .bind(opt_str!(profile, "lastname"))
        .bind(opt_str!(profile, "avatar"))
        .bind(if email_ok { email } else { None })
        .bind(crate::auth::service::now_ms())
        .execute(&sql)
        .await
        .map(|r| r.rows_affected())
        .unwrap_or(0);
        report.profile = applied > 0;
    }
    let files_entries: Option<Vec<Value>> = (|| {
        let mut idx_file = archive.by_name("files_index.json").ok()?;
        let mut s = String::new();
        if idx_file.size() > MAX_ENTRY_BYTES {
            return None;
        }
        idx_file.read_to_string(&mut s).ok()?;
        serde_json::from_str::<Vec<Value>>(&s).ok()
    })();
    if let Some(entries) = files_entries
        && let Some(storage) = &storage
    {
        // borrow released: entries were read in a bounded scope above
        // metadata of restored nodes carries the transformed_path keys.
        let transformed_by_id: std::collections::HashMap<Uuid, String> = nodes_raw
            .iter()
            .filter_map(|n| {
                let id = as_uuid(n.get("id"))?;
                let key = n
                    .get("metadata")
                    .and_then(|m| m.get("transformed_path"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())?;
                Some((id, key))
            })
            .collect();

        for e in entries {
            let Some(path) = as_string(e.get("path")) else {
                continue;
            };
            let Some(id) = as_uuid(e.get("id")) else {
                continue;
            };
            let mut zf = match archive.by_name(&path) {
                Ok(f) => f,
                Err(_) => {
                    report
                        .files_skipped
                        .push(format!("{id}:archive entry missing"));
                    continue;
                }
            };
            if zf.size() > MAX_ENTRY_BYTES {
                report.files_skipped.push(format!("{id}:entry too large"));
                continue;
            }
            let mut buf = Vec::with_capacity(zf.size() as usize);
            if zf.read_to_end(&mut buf).is_err() {
                report.files_skipped.push(format!("{id}:unreadable"));
                continue;
            }
            let ext = path
                .rsplit_once('.')
                .map(|(_, e)| format!(".{e}"))
                .unwrap_or_default();
            let _legacy_key = transformed_by_id
                .get(&id)
                .cloned()
                .unwrap_or_else(|| format!("{id}{ext}"));
            match crate::blobs::acquire(&sql, storage, &buf).await {
                Ok(sha) => {
                    // Point the restored node at the blob (metadata already
                    // carries the legacy transformed_path as fallback).
                    let _ = sqlx::query(
                        "UPDATE nodes SET metadata = jsonb_set(COALESCE(metadata, '{}'), '{content_sha256}', to_jsonb($1::text)) WHERE id = $2",
                    )
                    .bind(&sha)
                    .bind(id)
                    .execute(&sql)
                    .await;
                    report.files += 1;
                }
                Err(err) => report
                    .files_skipped
                    .push(format!("{id}:put failed ({err})")),
            }
        }
    }

    Ok(report)
}
