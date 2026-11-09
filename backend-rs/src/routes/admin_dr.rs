//! Admin-only instance DR management (spec backup-restore.md §10).
//!
//! Thin orchestration over the dr-backup sidecar: the backend reads/writes
//! the dr-backup.conf on a shared volume and exchanges job files over a
//! second shared volume. No network surface is added; the sidecar polls the
//! jobs directory and runs the actual pg_dump/rclone work outside the app
//! process (failure-domain separation, see spec §1/§10).

use axum::extract::{Path, State};
use serde::Deserialize;
use serde_json::json;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::response::ApiOk;

fn conf_dir() -> Option<std::path::PathBuf> {
    std::env::var("DR_CONF_DIR")
        .ok()
        .map(std::path::PathBuf::from)
}

fn jobs_dir() -> Option<std::path::PathBuf> {
    std::env::var("DR_JOBS_DIR")
        .ok()
        .map(std::path::PathBuf::from)
}

fn require_sidecar() -> Result<(std::path::PathBuf, std::path::PathBuf), ApiError> {
    match (conf_dir(), jobs_dir()) {
        (Some(c), Some(j)) if c.is_dir() && j.is_dir() => Ok((c, j)),
        _ => Err(ApiError::NotFound(
            "DR sidecar not deployed (shared volumes missing)".into(),
        )),
    }
}

fn require_admin(auth: &AuthUser) -> Result<(), ApiError> {
    if crate::auth::guard::is_app_admin(auth.role) {
        Ok(())
    } else {
        Err(ApiError::Forbidden("admin only".into()))
    }
}

const CONF_FILE: &str = "dr-backup.conf";
const MASK: &str = "***";

/// Masks secret fields in one pipe-delimited record line (pg/s3/webdav).
fn mask_line(line: &str) -> String {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('"') {
        return line.to_string(); // array brackets, blank and comment lines
    }
    let (Some(q1), Some(q2)) = (line.find('"'), line.rfind('"')) else {
        return line.to_string();
    };
    if q2 <= q1 {
        return line.to_string();
    }
    let mut fields: Vec<&str> = line[q1 + 1..q2].split('|').collect();
    match fields.first().copied() {
        Some("pg") if fields.len() >= 6 => fields[5] = MASK,
        Some("s3") if fields.len() >= 7 => {
            fields[5] = MASK;
            fields[6] = MASK;
        }
        Some("webdav") if fields.len() >= 5 => fields[4] = MASK,
        _ => return line.to_string(),
    }
    // Skip the closing quote itself (q2), otherwise it would be duplicated
    // and every masked record would read `"...""` on round-trip.
    format!(
        "{}\"{}\"{}",
        &line[..q1 + 1],
        fields.join("|"),
        &line[q2 + 1..]
    )
}

/// Restores `***` placeholders in `new_content` from the previous file so a
/// masked GET/PUT round-trip does not wipe real credentials.
fn unmask_content(new_content: &str, old_content: &str) -> Result<String, ApiError> {
    let old_by_key: std::collections::HashMap<(String, String), Vec<String>> = old_content
        .lines()
        .filter_map(|line| {
            let t = line.trim_start();
            if !t.starts_with('"') {
                return None;
            }
            let inner = line
                .trim()
                .trim_matches(|c| c == '"' || c == ',' || c == ' ');
            let fields: Vec<String> = inner.split('|').map(|s| s.to_string()).collect();
            if fields.len() < 2 {
                return None;
            }
            Some(((fields[0].clone(), fields[1].clone()), fields))
        })
        .collect();

    let mut out = Vec::new();
    let mut changed_masked = 0usize;
    for line in new_content.lines() {
        let t = line.trim_start();
        if t.starts_with('"') && t.contains('|') {
            let inner = t.trim().trim_matches(|c| c == '"' || c == ',' || c == ' ');
            let mut fields: Vec<String> = inner.split('|').map(|s| s.to_string()).collect();
            if fields.len() >= 2 {
                let secret_idx: Vec<usize> = match fields[0].as_str() {
                    "pg" if fields.len() >= 6 => vec![5],
                    "s3" if fields.len() >= 7 => vec![5, 6],
                    "webdav" if fields.len() >= 5 => vec![4],
                    _ => vec![],
                };
                let key = (fields[0].clone(), fields[1].clone());
                if let Some(old) = old_by_key.get(&key) {
                    for i in secret_idx {
                        if fields[i] == MASK && old.len() > i {
                            fields[i] = old[i].clone();
                            changed_masked += 1;
                        }
                    }
                } else if fields.iter().any(|f| f == MASK) {
                    return Err(ApiError::BadRequest(
                        "new record contains *** but no previous value to keep".into(),
                    ));
                }
                let quoted = format!("\"{}\"", fields.join("|"));
                let lead = line.len() - line.trim_start().len();
                out.push(format!("{}{}", " ".repeat(lead), quoted));
                continue;
            }
        }
        out.push(line.to_string());
    }
    let _ = changed_masked;
    let mut result = out.join("\n");
    if new_content.ends_with('\n') {
        result.push('\n');
    }
    Ok(result)
}

/// GET /api/admin/dr-backup/config
pub async fn get_config(
    State(_state): State<AppState>,
    auth: AuthUser,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    let (conf_dir, _) = require_sidecar()?;
    let path = conf_dir.join(CONF_FILE);
    if !path.exists() {
        return Ok(ApiOk::ok(json!({
            "exists": false,
            "content": "",
        })));
    }
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| ApiError::Internal(Box::new(std::io::Error::other(e.to_string()))))?;
    let masked: Vec<String> = raw.lines().map(mask_line).collect();
    Ok(ApiOk::ok(json!({
        "exists": true,
        "content": masked.join("\n"),
    })))
}

#[derive(Debug, Deserialize)]
pub struct PutConfigBody {
    pub content: String,
}

/// PUT /api/admin/dr-backup/config
pub async fn put_config(
    State(_state): State<AppState>,
    auth: AuthUser,
    body: axum::Json<PutConfigBody>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    let (conf_dir, _) = require_sidecar()?;
    let path = conf_dir.join(CONF_FILE);
    let old = std::fs::read_to_string(&path).unwrap_or_default();
    let content = unmask_content(&body.content, &old)?;
    std::fs::write(&path, content).map_err(|e| {
        ApiError::Internal(Box::new(std::io::Error::other(format!(
            "conf write failed: {e}"
        ))))
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    // Syntax check when a shell is available (backend image may lack bash).
    if let Ok(output) = std::process::Command::new("bash")
        .arg("-n")
        .arg(&path)
        .output()
        && !output.status.success()
    {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let first = stderr.lines().next().unwrap_or("").to_string();
        return Err(ApiError::BadRequest(format!("conf syntax error: {first}")));
    }
    Ok(ApiOk::ok(json!("Configuration saved.")))
}

#[derive(Debug, Deserialize)]
pub struct CreateJobBody {
    pub action: String, // backup | list | restore
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub ts: Option<String>,
}

/// POST /api/admin/dr-backup/jobs
pub async fn create_job(
    State(_state): State<AppState>,
    auth: AuthUser,
    body: axum::Json<CreateJobBody>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    let (_, jobs_dir) = require_sidecar()?;
    let action = body.action.as_str();
    let flags: Vec<String> = match action {
        "backup" => vec![],
        "list" => vec!["-l".into()],
        "restore" => {
            let Some(source) = body.source.as_deref().filter(|s| !s.is_empty()) else {
                return Err(ApiError::BadRequest(
                    "source is required for restore".into(),
                ));
            };
            let mut f = vec![format!("-R {source}")];
            if let Some(ts) = body.ts.as_deref().filter(|s| !s.is_empty()) {
                f.push(format!("-t {ts}"));
            }
            // admin + frontend double-confirm happen upstream; the sidecar is
            // non-interactive so -y is mandatory here.
            f.push("-y".into());
            f
        }
        other => return Err(ApiError::BadRequest(format!("unknown action: {other}"))),
    };
    let id = format!(
        "{}{:04}",
        crate::auth::service::now_ms(),
        (rand::random::<u32>() % 10000)
    );
    let pending = jobs_dir.join("pending");
    std::fs::create_dir_all(&pending)
        .map_err(|e| ApiError::Internal(Box::new(std::io::Error::other(e.to_string()))))?;
    let job = json!({
        "id": id,
        "action": action,
        "flags": flags,
        "requested_by": auth.user_id().to_string(),
        "created_at": crate::auth::service::now_ms(),
    });
    std::fs::write(
        pending.join(format!("{id}.json")),
        serde_json::to_vec(&job).unwrap_or_default(),
    )
    .map_err(|e| ApiError::Internal(Box::new(std::io::Error::other(e.to_string()))))?;
    Ok(ApiOk::ok(json!({ "job_id": id })))
}

/// GET /api/admin/dr-backup/jobs/:id
pub async fn get_job(
    State(_state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    let (_, jobs_dir) = require_sidecar()?;
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
        return Err(ApiError::BadRequest("invalid job id".into()));
    }
    let done = jobs_dir.join("done").join(format!("{id}.json"));
    if let Ok(bytes) = std::fs::read(&done) {
        let mut v: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|e| ApiError::Internal(Box::new(std::io::Error::other(e.to_string()))))?;
        v["status"] = json!("done");
        return Ok(ApiOk::ok(v));
    }
    let pending = jobs_dir.join("pending").join(format!("{id}.json"));
    if pending.exists() {
        return Ok(ApiOk::ok(json!({ "status": "pending", "id": id })));
    }
    Err(ApiError::NotFound("job not found".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"# dr-backup configuration
SOURCES=(
  "pg|pergame|localhost|5432|backup_user|secret-pass|pergame"
)
S3_DESTINATIONS=(
  "s3|primary|https://s3.example.com:9000|pergame-backups|pg|AK|SK|true|us-east-1|Other"
)
WEBDAV_DESTINATIONS=(
  "webdav|nas|https://nas/dav/|backup_user|dav-pass|pergame/pg"
)
RETAIN_DAYS=14"#;

    #[test]
    fn mask_and_unmask_roundtrip() {
        let masked = SAMPLE.lines().map(mask_line).collect::<Vec<_>>().join("\n");
        assert!(!masked.contains("secret-pass"), "pg pass must be masked");
        assert!(!masked.contains("|SK|"), "s3 secret must be masked");
        assert!(!masked.contains("dav-pass"), "webdav pass must be masked");
        assert!(!masked.contains("|AK|"), "s3 access key id is masked too");
        assert!(
            masked.contains("RETAIN_DAYS=14"),
            "non-record lines untouched"
        );
        // Regression: masked lines must not grow a duplicated closing quote.
        // Only record lines matter: each must end with exactly one quote.
        for line in masked.lines().filter(|l| l.trim_start().starts_with('"')) {
            assert!(
                line.trim_end().ends_with("\"") && !line.trim_end().ends_with("\"\""),
                "masked record must keep a single closing quote: {line}"
            );
        }

        let restored = unmask_content(&masked, SAMPLE).expect("unmask");
        assert!(restored.contains("secret-pass"), "pg pass restored");
        assert!(restored.contains("|SK|"), "s3 secret restored");
        assert!(restored.contains("dav-pass"), "webdav pass restored");
        assert_eq!(
            restored, SAMPLE,
            "unmask must reproduce the original exactly"
        );
    }

    #[test]
    fn unmask_rejects_new_record_with_placeholder() {
        let new_content = "  \"pg|new-db|host|5432|user|***|db\"";
        assert!(unmask_content(new_content, SAMPLE).is_err());
    }
}
