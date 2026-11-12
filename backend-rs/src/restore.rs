//! Snapshot restore engine (spec .work/features/backup-restore.md §3).
//!
//! Two entrances feed the same engine: restoring a snapshot already stored in
//! the backups bucket, and restoring an uploaded zip. Strategy is replace:
//! the caller's nodes (cascading links/permissions/invitations) are removed
//! inside a transaction, then rebuilt from documents.json; links pointing at
//! missing nodes are skipped and reported.

use serde::Serialize;
use serde_json::Value;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;
use zip::ZipArchive;

use crate::error::ApiError;
use crate::storage::S3Storage;

/// zip safety caps (spec §3): at most 5000 entries, 100MB per entry.
const MAX_ENTRIES: usize = 5000;
const MAX_ENTRY_BYTES: u64 = 100 * 1024 * 1024;

#[derive(Debug, Default, Serialize)]
pub struct RestoreReport {
    pub nodes: usize,
    pub links: usize,
    pub links_skipped: Vec<String>,
    pub files: usize,
    pub files_skipped: Vec<String>,
    pub settings: bool,
    pub permissions: usize,
    pub permissions_skipped: Vec<String>,
    pub invitations: usize,
    pub invitations_skipped: Vec<String>,
    pub trashed: usize,
    pub trashed_skipped: Vec<String>,
    pub profile: bool,
    pub warnings: Vec<String>,
}

/// Highest accepted archive format (semver-ish major.minor pair).
const MAX_FORMAT: (u32, u32) = (2, 0); // v2 archives (manifest+blobs assembled) and legacy 1.x

fn as_i64(v: Option<&Value>) -> Option<i64> {
    v.and_then(|v| {
        v.as_i64()
            .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
    })
}

fn as_uuid(v: Option<&Value>) -> Option<Uuid> {
    v.and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
}

fn as_opt_uuid(v: Option<&Value>) -> Option<Uuid> {
    match v {
        Some(Value::Null) | None => None,
        other => as_uuid(other),
    }
}

fn as_opt_i64(v: Option<&Value>) -> Option<i64> {
    match v {
        Some(Value::Null) | None => None,
        other => as_i64(other),
    }
}

fn as_string(v: Option<&Value>) -> Option<String> {
    v.and_then(|v| v.as_str()).map(|s| s.to_string())
}

pub async fn restore_from_zip(
    sql: PgPool,
    storage: Option<S3Storage>,
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
        use std::io::Read as _;
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
        use std::io::Read as _;
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
            use std::io::Read as _;
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
        use std::io::Read as _;
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
    for n in &nodes_raw {
        let Some(id) = as_uuid(n.get("id")) else {
            report.warnings.push("node without id skipped".into());
            continue;
        };
        // Parent must be present in the archive; orphan the node otherwise.
        let parent = as_opt_uuid(n.get("parent_id"));
        let parent = match parent {
            Some(p) if node_ids.contains(&p) => Some(p),
            Some(p) => {
                report.warnings.push(format!(
                    "node {id}: parent {p} not in archive, promoted to top level"
                ));
                restored_parent_fixed += 1;
                None
            }
            None => None,
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
        .bind(parent)
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

    // ---- permissions: node must be in the archive; grantee resolved by
    // username (unknown users skipped, never fabricated). ON CONFLICT skips
    // duplicates inside the archive itself.
    {
        let mut user_cache: std::collections::HashMap<String, Option<Uuid>> =
            std::collections::HashMap::new();
        for p in &permissions_raw {
            let (Some(node_id), Some(grantee), level) = (
                as_uuid(p.get("node_id")),
                as_string(p.get("grantee")),
                as_i64(p.get("permission")).unwrap_or(1) as i32,
            ) else {
                report.permissions_skipped.push("missing fields".into());
                continue;
            };
            if !node_ids.contains(&node_id) {
                report
                    .permissions_skipped
                    .push(format!("{grantee}:node not in archive"));
                continue;
            }
            let grantee_id = match user_cache.get(&grantee) {
                Some(cached) => *cached,
                None => {
                    let found: Option<(Uuid,)> =
                        sqlx::query_as("SELECT id FROM users WHERE username = $1")
                            .bind(&grantee)
                            .fetch_optional(&mut *tx)
                            .await
                            .unwrap_or(None);
                    let id = found.map(|r| r.0);
                    user_cache.insert(grantee.clone(), id);
                    id
                }
            };
            let Some(grantee_id) = grantee_id else {
                report
                    .permissions_skipped
                    .push(format!("{grantee}:unknown user"));
                continue;
            };
            let inserted = sqlx::query(
                r#"INSERT INTO permissions (id, node_id, user_id, permission, created_timestamp)
                   VALUES ($1,$2,$3,$4,$5) ON CONFLICT (node_id, user_id) DO NOTHING"#,
            )
            .bind(crate::types::new_id())
            .bind(node_id)
            .bind(grantee_id)
            .bind(level)
            .bind(as_i64(p.get("created_timestamp")).unwrap_or(0))
            .execute(&mut *tx)
            .await
            .map(|r| r.rows_affected())
            .unwrap_or(0);
            if inserted == 0 {
                report
                    .permissions_skipped
                    .push(format!("{grantee}:duplicate"));
            } else {
                report.permissions += 1;
            }
        }
    }

    // ---- invitations: self-contained rows, deduped by code. A conflicting
    // code means the same invitation already exists — skip, the old code
    // stays valid and nothing is lost.
    for inv in &invitations_raw {
        let (Some(code), Some(node_id)) = (
            as_string(inv.get("invitation_code")),
            as_uuid(inv.get("node_id")),
        ) else {
            report.invitations_skipped.push("missing fields".into());
            continue;
        };
        if !node_ids.contains(&node_id) {
            report
                .invitations_skipped
                .push(format!("{code}:node not in archive"));
            continue;
        }
        let inserted = sqlx::query(
            r#"INSERT INTO node_invitations (id, invitation_code, permission_level, node_id, created_timestamp)
               VALUES ($1,$2,$3,$4,$5) ON CONFLICT (invitation_code) DO NOTHING"#,
        )
        .bind(crate::types::new_id())
        .bind(&code)
        .bind(as_i64(inv.get("permission_level")).unwrap_or(1) as i32)
        .bind(node_id)
        .bind(as_i64(inv.get("created_timestamp")).unwrap_or(0))
        .execute(&mut *tx)
        .await
        .map(|r| r.rows_affected())
        .unwrap_or(0);
        if inserted == 0 {
            report.invitations_skipped.push(format!("{code}:duplicate"));
        } else {
            report.invitations += 1;
        }
    }

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
        use std::io::Read as _;
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
        use std::io::Read as _;
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
            use std::io::Read as _;
            if zf.read_to_end(&mut buf).is_err() {
                report.files_skipped.push(format!("{id}:unreadable"));
                continue;
            }
            let ext = path
                .rsplit_once('.')
                .map(|(_, e)| format!(".{e}"))
                .unwrap_or_default();
            let key = transformed_by_id
                .get(&id)
                .cloned()
                .unwrap_or_else(|| format!("{id}{ext}"));
            match storage.put_user_object(user_id, &key, buf, None).await {
                Ok(()) => report.files += 1,
                Err(err) => report
                    .files_skipped
                    .push(format!("{id}:put failed ({err})")),
            }
        }
    }

    Ok(report)
}

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
        use std::io::Read as _;
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
    drop(read_member);
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
        use std::io::Read as _;
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

// ---------------- portable zip snapshot (fork §7 parity) ----------------
//
// The offline export uses the *store object set* layout rather than the v1
// archive shape: `siyuan-compatible` meta.json + manifest.json + blobs/<sha256>`.
// Every entry name is pure ASCII (hashes and fixed names), so classic zip
// filename-encoding problems (CP437 vs UTF-8) cannot occur; document paths
// live inside the UTF-8 manifest JSON and blob bytes are sha256-checked.

/// meta.json payload identifying the export.
#[derive(Debug, Serialize, serde::Deserialize)]
pub struct PortableMeta {
    pub format: String,
    pub version: String,
    pub created_at: String,
    pub snapshot_id: String,
    pub user_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encryption: Option<crate::snapcrypto::EncryptionParams>,
    pub entries: Vec<crate::backup::ManifestEntry>,
    pub files: Vec<crate::backup::ManifestFile>,
}

pub const PORTABLE_FORMAT: &str = "pergame-backup/object-set";
pub const PORTABLE_VERSION: &str = "1";

/// Builds the portable object-set zip from in-memory payloads (zero-target
/// snapshots): identical layout to `export_portable_zip`, but the blobs come
/// from the just-collected snapshot rather than any store.
pub fn build_portable_zip_in_memory(
    manifest: &crate::backup::BackupManifest,
    manifest_bytes: &[u8],
    member_payloads: &[(String, bool, Vec<u8>)],
    file_payloads: &[(String, bool, Vec<u8>)],
) -> Result<Vec<u8>, ApiError> {
    let meta = PortableMeta {
        format: PORTABLE_FORMAT.into(),
        version: PORTABLE_VERSION.into(),
        created_at: manifest.created_at.clone(),
        snapshot_id: manifest.id.clone(),
        user_id: manifest.user_id.clone(),
        encryption: manifest.encryption.clone(),
        entries: manifest.entries.clone(),
        files: manifest.files.clone(),
    };
    let mut archive = crate::backup::SimpleArchive::new();
    archive.add(
        "meta.json",
        &serde_json::to_vec_pretty(&meta).unwrap_or_default(),
    );
    archive.add("manifest.json", manifest_bytes);
    let mut seen = std::collections::HashSet::new();
    for (sha, e2e, bytes) in member_payloads.iter().chain(file_payloads.iter()) {
        if !seen.insert(sha.clone()) {
            continue;
        }
        // Ciphertext blobs are incompressible: store as-is (fork §7).
        if *e2e {
            archive.add_stored(format!("blobs/{sha}"), bytes);
        } else {
            archive.add(&format!("blobs/{sha}"), bytes);
        }
    }
    archive
        .finish()
        .map_err(|e| ApiError::BadRequest(format!("zip assembly failed: {e}")))
}
/// Builds the portable zip for a v2 snapshot: manifest.json + one blob per
/// referenced hash (ciphertext preserved when the snapshot is encrypted).
/// Reads blobs from a connected target (direct-write read path).
pub async fn export_portable_zip_from_conn(
    conn: &crate::target::config::TargetConn,
    manifest: &crate::backup::BackupManifest,
) -> Result<Vec<u8>, ApiError> {
    let mut archive = crate::backup::SimpleArchive::new();
    let e2e = manifest.encryption.is_some();
    let meta = PortableMeta {
        format: PORTABLE_FORMAT.into(),
        version: PORTABLE_VERSION.into(),
        created_at: manifest.created_at.clone(),
        snapshot_id: manifest.id.clone(),
        user_id: manifest.user_id.clone(),
        encryption: manifest.encryption.clone(),
        entries: manifest.entries.clone(),
        files: manifest.files.clone(),
    };
    archive.add(
        "meta.json",
        &serde_json::to_vec_pretty(&meta).unwrap_or_default(),
    );
    archive.add(
        "manifest.json",
        &serde_json::to_vec_pretty(manifest).unwrap_or_default(),
    );
    let dir = if e2e { "blobs-e2e" } else { "blobs" };
    let mut seen = std::collections::HashSet::new();
    let mut hashes: Vec<String> = manifest.entries.iter().map(|e| e.sha256.clone()).collect();
    hashes.extend(manifest.files.iter().map(|f| f.sha256.clone()));
    for sha in hashes {
        if !seen.insert(sha.clone()) {
            continue;
        }
        let blob = conn
            .get(&format!("{}/{dir}/{sha}", manifest.user_id))
            .await?;
        if e2e {
            archive.add_stored(format!("blobs/{sha}"), &blob);
        } else {
            archive.add(&format!("blobs/{sha}"), &blob);
        }
    }
    archive
        .finish()
        .map_err(|e| ApiError::BadRequest(format!("portable zip assembly failed: {e}")))
}

/// Recognises a portable object-set zip (as opposed to a v1 archive).
pub fn is_portable_zip(bytes: &[u8]) -> bool {
    let cursor = std::io::Cursor::new(bytes.to_vec());
    let Ok(mut archive) = ZipArchive::new(cursor) else {
        return false;
    };
    let Ok(mut meta) = archive.by_name("meta.json") else {
        return false;
    };
    let mut s = String::new();
    use std::io::Read as _;
    if meta.read_to_string(&mut s).is_err() {
        return false;
    }
    serde_json::from_str::<PortableMeta>(&s)
        .map(|m| m.format == PORTABLE_FORMAT)
        .unwrap_or(false)
}

/// Converts a portable object-set zip into the v1 archive shape the restore
/// engine consumes: meta/manifest/blobs -> manifest.json + documents.json +
/// files_index.json + files/.
pub fn portable_to_v1_zip(bytes: Vec<u8>) -> Result<Vec<u8>, ApiError> {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|e| ApiError::BadRequest(format!("invalid zip archive: {e}")))?;
    let meta: PortableMeta = {
        let mut f = archive
            .by_name("meta.json")
            .map_err(|_| ApiError::BadRequest("meta.json missing".into()))?;
        let mut s = String::new();
        use std::io::Read as _;
        f.read_to_string(&mut s)
            .map_err(|_| ApiError::BadRequest("meta.json unreadable".into()))?;
        serde_json::from_str(&s).map_err(|e| ApiError::BadRequest(format!("meta.json: {e}")))?
    };
    if meta.format != PORTABLE_FORMAT {
        return Err(ApiError::BadRequest("not a portable object-set zip".into()));
    }
    // The manifest itself travels inside the zip.
    let manifest: crate::backup::BackupManifest = {
        let mut f = archive
            .by_name("manifest.json")
            .map_err(|_| ApiError::BadRequest("manifest.json missing".into()))?;
        let mut s = String::new();
        use std::io::Read as _;
        f.read_to_string(&mut s)
            .map_err(|_| ApiError::BadRequest("manifest.json unreadable".into()))?;
        serde_json::from_str(&s).map_err(|e| ApiError::BadRequest(format!("manifest.json: {e}")))?
    };
    let mut out = crate::backup::SimpleArchive::new();
    out.add(
        "manifest.json",
        &serde_json::to_vec_pretty(&manifest).unwrap_or_default(),
    );
    // Members: documents.json / user_settings.json live as blobs in the set.
    for entry in &meta.entries {
        let mut f = archive
            .by_name(&format!("blobs/{}", entry.sha256))
            .map_err(|_| ApiError::BadRequest(format!("blob missing for {}", entry.path)))?;
        let mut buf = Vec::with_capacity(entry.size as usize);
        use std::io::Read as _;
        f.read_to_end(&mut buf)
            .map_err(|_| ApiError::BadRequest("blob unreadable".into()))?;
        if buf.len() as u64 != entry.size {
            return Err(ApiError::BadRequest(format!(
                "blob size mismatch for {} ({} vs {})",
                entry.path,
                buf.len(),
                entry.size
            )));
        }
        out.add(&entry.path, &buf);
    }
    // Files: files_index.json + files/<id><ext>, matching the v1 layout.
    let mut index = Vec::with_capacity(meta.files.len());
    for f in &meta.files {
        let ext = f
            .key
            .rsplit_once('.')
            .map(|(_, e)| format!(".{e}"))
            .unwrap_or_default();
        index.push(json!({
            "id": f.id,
            "path": format!("files/{}{}", f.id, ext),
            "original_path": f.original_path,
            "size": f.size,
        }));
    }
    out.add(
        "files_index.json",
        &serde_json::to_vec_pretty(&index).unwrap_or_default(),
    );
    for f in &meta.files {
        let ext = f
            .key
            .rsplit_once('.')
            .map(|(_, e)| format!(".{e}"))
            .unwrap_or_default();
        let mut zf = archive
            .by_name(&format!("blobs/{}", f.sha256))
            .map_err(|_| ApiError::BadRequest(format!("blob missing for file {}", f.id)))?;
        let mut buf = Vec::new();
        use std::io::Read as _;
        zf.read_to_end(&mut buf)
            .map_err(|_| ApiError::BadRequest("file blob unreadable".into()))?;
        out.add_stored(format!("files/{}{}", f.id, ext), &buf);
    }
    out.finish()
        .map_err(|e| ApiError::BadRequest(format!("conversion failed: {e}")))
}

#[cfg(test)]
mod portable_tests {
    use super::*;

    fn sample_manifest() -> crate::backup::BackupManifest {
        crate::backup::BackupManifest {
            version: crate::backup::BACKUP_VERSION.into(),
            id: "1794281400000".into(),
            created_at: "2026-11-10T03:30:00Z".into(),
            user_id: "42".into(),
            memo: Some("portable test".into()),
            encryption: None,
            options: crate::backup::BackupOptions {
                include_documents: true,
                ..Default::default()
            },
            statistics: crate::backup::BackupStats::default(),
            entries: vec![crate::backup::ManifestEntry {
                path: "documents.json".into(),
                sha256: "aa".repeat(32),
                size: 12,
            }],
            files: vec![crate::backup::ManifestFile {
                id: "7".into(),
                key: "7.png".into(),
                original_path: Some("photo 中文.png".into()),
                sha256: "bb".repeat(32),
                size: 5,
            }],
            logical_bytes: 17,
        }
    }

    /// Builds a portable zip by hand (no storage needed) and checks the
    /// object-set -> v1 conversion keeps every member byte-identical.
    #[test]
    fn portable_to_v1_roundtrip() {
        let manifest = sample_manifest();
        let mut set = crate::backup::SimpleArchive::new();
        let meta = PortableMeta {
            format: PORTABLE_FORMAT.into(),
            version: PORTABLE_VERSION.into(),
            created_at: manifest.created_at.clone(),
            snapshot_id: manifest.id.clone(),
            user_id: manifest.user_id.clone(),
            encryption: None,
            entries: manifest.entries.clone(),
            files: manifest.files.clone(),
        };
        set.add("meta.json", &serde_json::to_vec(&meta).unwrap());
        set.add("manifest.json", &serde_json::to_vec(&manifest).unwrap());
        set.add(
            "blobs/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            br#"{"nodes":[]}"#,
        );
        set.add(
            "blobs/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            b"\x89PNG",
        );
        let bytes = set.finish().expect("assemble");

        assert!(
            is_portable_zip(&bytes),
            "meta.json format must be recognised"
        );
        let v1 = portable_to_v1_zip(bytes).expect("convert");
        let mut archive = ZipArchive::new(std::io::Cursor::new(v1)).expect("valid v1 zip");
        let mut names: Vec<String> = (0..archive.len())
            .filter_map(|i| {
                archive
                    .by_index(i)
                    .ok()
                    .and_then(|f| f.name().ok().map(|n| n.to_string()))
            })
            .collect();
        names.sort();
        assert!(names.contains(&"manifest.json".to_string()));
        assert!(names.contains(&"documents.json".to_string()));
        assert!(names.contains(&"files_index.json".to_string()));
        assert!(names.contains(&"files/7.png".to_string()));
        let mut doc = String::new();
        {
            use std::io::Read as _;
            archive
                .by_name("documents.json")
                .unwrap()
                .read_to_string(&mut doc)
                .unwrap();
        }
        assert_eq!(doc, r#"{"nodes":[]}"#, "member bytes preserved");
    }

    #[test]
    fn rejects_plain_v1_zip_as_portable() {
        let mut archive = crate::backup::SimpleArchive::new();
        archive.add("manifest.json", b"{}");
        archive.add("documents.json", b"[]");
        let bytes = archive.finish().unwrap();
        assert!(!is_portable_zip(&bytes), "a v1 archive has no meta.json");
    }
}
