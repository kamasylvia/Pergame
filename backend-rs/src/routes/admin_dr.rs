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

fn sidecar_dirs() -> Option<(std::path::PathBuf, std::path::PathBuf)> {
    match (conf_dir(), jobs_dir()) {
        (Some(c), Some(j)) if c.is_dir() && j.is_dir() => Some((c, j)),
        _ => None,
    }
}

fn require_sidecar() -> Result<(std::path::PathBuf, std::path::PathBuf), ApiError> {
    match (conf_dir(), jobs_dir()) {
        (Some(c), Some(j)) if c.is_dir() && j.is_dir() => Ok((c, j)),
        _ => Err(ApiError::NotFound(
            "DR sidecar not deployed (shared volumes missing)".into(),
        )),
    }
}

/// Names + cron expressions of every destination that opted into scheduling
/// (cron field non-empty in its record). A name present in both kinds fires
/// both records, mirroring the `-T` filter's matching rule.
fn destination_crons(cfg: &crate::routes::dr_config::DrConfig) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for d in &cfg.s3_destinations {
        if !d.cron.trim().is_empty() {
            out.push((d.name.clone(), d.cron.trim().to_string()));
        }
    }
    for d in &cfg.webdav_destinations {
        if !d.cron.trim().is_empty() {
            out.push((d.name.clone(), d.cron.trim().to_string()));
        }
    }
    out
}

/// True when a pending job already targets `name` (the sidecar executes jobs
/// serially and deletes pending files when done, so an existing file means the
/// previous tick's backup is still queued or running — skip, no queue storm).
fn pending_job_for_target(jobs_dir: &std::path::Path, name: &str) -> bool {
    let pending = jobs_dir.join("pending");
    let Ok(entries) = std::fs::read_dir(&pending) else {
        return false;
    };
    for entry in entries.flatten() {
        let Ok(bytes) = std::fs::read(entry.path()) else {
            continue;
        };
        let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
            continue;
        };
        if v["target"].as_str() == Some(name) {
            return true;
        }
    }
    false
}

fn write_pending_job(
    jobs_dir: &std::path::Path,
    action: &str,
    flags: Vec<String>,
    target: Option<String>,
) -> Result<String, std::io::Error> {
    let id = format!(
        "{}{:04}",
        crate::auth::service::now_ms(),
        (rand::random::<u32>() % 10000)
    );
    let pending = jobs_dir.join("pending");
    std::fs::create_dir_all(&pending)?;
    let mut job = json!({
        "id": id,
        "action": action,
        "flags": flags,
        "requested_by": "scheduler",
        "created_at": crate::auth::service::now_ms(),
    });
    if let Some(t) = target {
        job["target"] = json!(t);
    }
    std::fs::write(
        pending.join(format!("{id}.json")),
        serde_json::to_vec(&job).unwrap_or_default(),
    )?;
    Ok(id)
}

/// Per-destination cron scheduler (Siyuan scheduler parity). Every minute:
/// re-read the conf (config edits apply live), and for each destination whose
/// cron produced an occurrence within (last_check, now] write a
/// `dr-backup.sh -T <name>` job file. Missed ticks are never re-run and
/// restarts reset the in-memory tick state — standard cron semantics, same as
/// the Siyuan kernel scheduler.
pub fn spawn_dr_cron_scheduler(state: crate::AppState) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(60));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        ticker.tick().await; // fire immediately once so a fresh boot runs due jobs
        let mut last_check = chrono::Utc::now();
        loop {
            ticker.tick().await;
            let now = chrono::Utc::now();
            let Some((conf_path, jobs)) = sidecar_dirs().map(|(c, j)| (c.join(CONF_FILE), j))
            else {
                last_check = now;
                continue;
            };
            let Ok(raw) = std::fs::read_to_string(&conf_path) else {
                last_check = now;
                continue;
            };
            let Ok(cfg) = crate::routes::dr_config::DrConfig::from_conf(&raw) else {
                tracing::warn!("dr cron: conf unparseable; tick skipped");
                last_check = now;
                continue;
            };
            let _ = &state; // reserved for future per-run guards
            for (name, expr) in destination_crons(&cfg) {
                let Ok(schedule) = crate::routes::dr_config::parse_cron(&expr) else {
                    // Validation catches this at save time; skip at runtime.
                    continue;
                };
                let due = matches!(
                    schedule.find_next_occurrence(&last_check, false),
                    Ok(next) if next <= now
                );
                if !due {
                    continue;
                }
                if pending_job_for_target(&jobs, &name) {
                    tracing::debug!(target = %name, "dr cron: previous job still queued; skipped");
                    continue;
                }
                match write_pending_job(
                    &jobs,
                    "backup",
                    vec!["-T".into(), name.clone()],
                    Some(name.clone()),
                ) {
                    Ok(id) => tracing::info!(target = %name, job = %id, "dr cron fired backup"),
                    Err(e) => {
                        tracing::error!(target = %name, error = %e, "dr cron: job write failed")
                    }
                }
            }
            last_check = now;
        }
    });
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

/// Writes the conf and runs the bash syntax gate. Shared by the raw editor and
/// the structured form so both paths get the same protection.
fn write_conf(path: &std::path::Path, content: &str) -> Result<(), ApiError> {
    std::fs::write(path, content).map_err(|e| {
        ApiError::Internal(Box::new(std::io::Error::other(format!(
            "conf write failed: {e}"
        ))))
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    // Syntax check when a shell is available (backend image may lack bash).
    if let Ok(output) = std::process::Command::new("bash")
        .arg("-n")
        .arg(path)
        .output()
        && !output.status.success()
    {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let first = stderr.lines().next().unwrap_or("").to_string();
        return Err(ApiError::BadRequest(format!("conf syntax error: {first}")));
    }
    Ok(())
}

/// GET /api/admin/dr-backup/config/structured — the same conf as editable JSON.
/// Secrets come back masked; the UI never sees real credentials.
pub async fn get_structured_config(
    State(_state): State<AppState>,
    auth: AuthUser,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    let (conf_dir, _) = require_sidecar()?;
    let path = conf_dir.join(CONF_FILE);
    if !path.exists() {
        return Ok(ApiOk::ok(json!({
            "exists": false,
            "config": crate::routes::dr_config::DrConfig::default(),
        })));
    }
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| ApiError::Internal(Box::new(std::io::Error::other(e.to_string()))))?;
    let config =
        crate::routes::dr_config::DrConfig::from_conf(&raw).map_err(ApiError::BadRequest)?;
    Ok(ApiOk::ok(json!({ "exists": true, "config": config })))
}

/// PUT /api/admin/dr-backup/config/structured — validates, rehydrates masked
/// secrets from the file on disk, then renders and writes the bash conf.
pub async fn put_structured_config(
    State(_state): State<AppState>,
    auth: AuthUser,
    body: axum::Json<crate::routes::dr_config::DrConfig>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    let (conf_dir, _) = require_sidecar()?;
    let path = conf_dir.join(CONF_FILE);
    let mut incoming = body.0;

    let old_raw = std::fs::read_to_string(&path).unwrap_or_default();
    if !old_raw.trim().is_empty() {
        let previous = crate::routes::dr_config::DrConfig::from_conf(&old_raw)
            .map_err(ApiError::BadRequest)?;
        // from_conf masks secrets; unmask it against the raw file so the old
        // values are available for the fill step below.
        let mut previous = previous;
        previous.sources.iter_mut().for_each(|s| {
            if let Some(raw_pass) = raw_secret(&old_raw, "pg", &s.name, 5) {
                s.password = raw_pass;
            }
        });
        previous.s3_destinations.iter_mut().for_each(|d| {
            if let Some(v) = raw_secret(&old_raw, "s3", &d.name, 5) {
                d.access_key = v;
            }
            if let Some(v) = raw_secret(&old_raw, "s3", &d.name, 6) {
                d.secret_key = v;
            }
            // Field 13 (index 13) = crypt_pass; only rehydrate when the old
            // record actually carries it (old confs end at index 9).
            if let Some(v) = raw_secret(&old_raw, "s3", &d.name, 13) {
                d.crypt_pass = v;
            }
        });
        previous.webdav_destinations.iter_mut().for_each(|d| {
            if let Some(v) = raw_secret(&old_raw, "webdav", &d.name, 4) {
                d.password = v;
            }
            if let Some(v) = raw_secret(&old_raw, "webdav", &d.name, 9) {
                d.crypt_pass = v;
            }
        });
        incoming
            .fill_secrets(&previous)
            .map_err(ApiError::BadRequest)?;
    }

    incoming.validate().map_err(ApiError::BadRequest)?;
    let rendered = incoming.to_conf();
    write_conf(&path, &rendered)?;
    Ok(ApiOk::ok(json!("Configuration saved.")))
}

/// Reads one field out of the raw (unmasked) conf for the named record.
fn raw_secret(raw: &str, kind: &str, name: &str, index: usize) -> Option<String> {
    raw.lines()
        .map(|l| l.trim())
        .filter(|l| l.starts_with('"'))
        .map(|l| l.trim_matches(|c| c == '"' || c == ','))
        .map(|record| record.split('|').map(|s| s.to_string()).collect::<Vec<_>>())
        .find(|fields| {
            fields.len() > index && fields[0] == kind && fields.get(1).is_some_and(|n| n == name)
        })
        .map(|fields| fields[index].clone())
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
    write_conf(&path, &content)?;
    Ok(ApiOk::ok(json!("Configuration saved.")))
}

#[derive(Debug, Deserialize)]
pub struct CreateJobBody {
    pub action: String, // backup | list | restore | probe
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub ts: Option<String>,
    /// Target name for the probe action (destination or source).
    #[serde(default)]
    pub name: Option<String>,
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
    let mut target = None;
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
        // UI "test connection": dr-backup.sh -p <name> prints one PROBE| line.
        "probe" => {
            let Some(name) = body.name.as_deref().filter(|s| !s.is_empty()) else {
                return Err(ApiError::BadRequest("name is required for probe".into()));
            };
            if name.contains(' ') {
                return Err(ApiError::BadRequest("name must not contain spaces".into()));
            }
            target = Some(name.to_string());
            vec!["-p".into(), name.to_string()]
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
    let mut job = json!({
        "id": id,
        "action": action,
        "flags": flags,
        "requested_by": auth.user_id().to_string(),
        "created_at": crate::auth::service::now_ms(),
    });
    if let Some(t) = &target {
        job["target"] = json!(t);
    }
    std::fs::write(
        pending.join(format!("{id}.json")),
        serde_json::to_vec(&job).unwrap_or_default(),
    )
    .map_err(|e| ApiError::Internal(Box::new(std::io::Error::other(e.to_string()))))?;
    Ok(ApiOk::ok(json!({ "job_id": id })))
}

/// Extracts the last `PROBE|...` line from job output into a structured value
/// (dr-backup.sh -p output; see probe_target there for the line format).
pub fn parse_probe_line(output: &str) -> Option<serde_json::Value> {
    let line = output
        .lines()
        .rev()
        .find(|l| l.trim_start().starts_with("PROBE|"))?;
    let f: Vec<&str> = line.trim_start().split('|').collect();
    if f.len() < 6 || f[0] != "PROBE" {
        return None;
    }
    Some(json!({
        "kind": f[1],
        "name": f[2],
        "reachable": f[3].strip_prefix("reachable=").unwrap_or(f[3]),
        "snapshots": f[4]
            .strip_prefix("snapshots=")
            .and_then(|v| v.parse::<i64>().ok()),
        "fingerprint": f[5].strip_prefix("fingerprint=").unwrap_or(f[5]),
        "note": f[6..].join("|").strip_prefix("note=").unwrap_or(""),
    }))
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
        // Probe jobs carry their machine-readable result inline.
        if let Some(tail) = v["output_tail"].as_str()
            && let Some(probe) = parse_probe_line(tail)
        {
            v["probe"] = probe;
        }
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

    #[test]
    fn probe_line_parses_into_structured_value() {
        let out = "[2026-11-09 20:25:02] some log line\n\
                   PROBE|s3|primary|reachable=true|snapshots=3|fingerprint=s3,192.0.2.1:9000,bk,pg|note=\n";
        let v = parse_probe_line(out).expect("probe line");
        assert_eq!(v["kind"], "s3");
        assert_eq!(v["name"], "primary");
        assert_eq!(v["reachable"], "true");
        assert_eq!(v["snapshots"], 3);
        assert_eq!(v["fingerprint"], "s3,192.0.2.1:9000,bk,pg");
        assert_eq!(v["note"], "");
    }

    #[test]
    fn probe_line_failure_and_garbage() {
        let out = "PROBE|pg|db1|reachable=false|snapshots=|fingerprint=|note=psql connect failed\n";
        let v = parse_probe_line(out).expect("probe line");
        assert_eq!(v["reachable"], "false");
        assert_eq!(v["snapshots"], serde_json::Value::Null);
        assert_eq!(v["note"], "psql connect failed");
        assert!(parse_probe_line("no probe output here").is_none());
        assert!(parse_probe_line("PROBE|short").is_none());
    }
}

// ---------------- fan-out targets (spec §14) ----------------

use crate::target::{fanout, repo as target_repo};

/// GET /api/admin/dr-backup/targets — list (secrets masked).
pub async fn list_targets(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    let rows = target_repo::list_targets(&state.db).await?;
    let masked: Vec<serde_json::Value> = rows.iter().map(target_repo::mask_row).collect();
    Ok(ApiOk::ok(json!(masked)))
}

/// POST /api/admin/dr-backup/targets — create.
pub async fn create_target(
    State(state): State<AppState>,
    auth: AuthUser,
    body: axum::Json<target_repo::TargetInput>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    let row = target_repo::upsert_target(&state.db, None, &body).await?;
    Ok(ApiOk::ok(target_repo::mask_row(&row)))
}

/// PUT /api/admin/dr-backup/targets/:id — edit (gated on connection changes).
pub async fn update_target(
    State(state): State<AppState>,
    auth: AuthUser,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
    body: axum::Json<target_repo::TargetInput>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    let row = target_repo::upsert_target(&state.db, Some(id), &body).await?;
    Ok(ApiOk::ok(target_repo::mask_row(&row)))
}

/// POST /api/admin/dr-backup/targets/:id/delete-check — probe before delete.
pub async fn delete_check_target(
    State(state): State<AppState>,
    auth: AuthUser,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    let row = target_repo::get_target(&state.db, id).await?;
    let probe = target_repo::probe_row(&row).await?;
    target_repo::delete_check(&state.db, id).await?;
    Ok(ApiOk::ok(json!({
        "deletable": true,
        "reachable": probe.reachable,
        "snapshot_objects": probe.snapshot_objects,
    })))
}

/// DELETE /api/admin/dr-backup/targets/:id — gated delete.
pub async fn delete_target(
    State(state): State<AppState>,
    auth: AuthUser,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    target_repo::delete_check(&state.db, id).await?;
    sqlx::query("DELETE FROM backup_targets WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(ApiError::from)?;
    Ok(ApiOk::ok(json!("Target deleted.")))
}

/// POST /api/admin/dr-backup/targets/:id/test — connectivity probe.
pub async fn test_target(
    State(state): State<AppState>,
    auth: AuthUser,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    let row = target_repo::get_target(&state.db, id).await?;
    let probe = target_repo::probe_row(&row).await?;
    Ok(ApiOk::ok(json!({
        "reachable": probe.reachable,
        "snapshot_objects": probe.snapshot_objects,
        "fingerprint": probe.fingerprint.map(|f| f.0),
        "owner_instance": probe.owner_instance,
        "owned_by_me": probe.owned_by_me,
    })))
}

/// POST /api/admin/dr-backup/targets/probe-draft — probe connection fields that
/// have not been saved yet (siyuan #15 §9: the save button stays disabled until
/// a test passes). `***` credentials are resolved against the stored row when
/// `id` is supplied, so an untouched password still tests the real store.
///
/// Returns a `connection_token` the client echoes back on save; the server
/// recomputes it there to prove the tested connection is the saved one.
pub async fn probe_draft_target(
    State(state): State<AppState>,
    auth: AuthUser,
    axum::Json(body): axum::Json<ProbeDraftBody>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    let mut input = body.target;
    // Rehydrate masked secrets from the stored row (edit mode).
    if let Some(id) = body.id {
        let old = target_repo::get_target(&state.db, id).await?;
        input.access_key = target_repo::resolve_secret_pub(&input.access_key, &old.access_key);
        input.secret_key = target_repo::resolve_secret_pub(&input.secret_key, &old.secret_key);
    } else if input.access_key == target_repo::MASK || input.secret_key == target_repo::MASK {
        return Err(ApiError::BadRequest(
            "new target cannot test with *** credentials".into(),
        ));
    }
    let candidate = target_repo::row_from_input_pub(&input, crate::auth::service::now_ms(), body.id);
    let probe = target_repo::probe_row(&candidate).await?;
    Ok(ApiOk::ok(json!({
        "reachable": probe.reachable,
        "snapshot_objects": probe.snapshot_objects,
        "fingerprint": probe.fingerprint.map(|f| f.0),
        "owner_instance": probe.owner_instance,
        "owned_by_me": probe.owned_by_me,
        "connection_token": target_repo::connection_token(&input),
    })))
}

#[derive(Debug, serde::Deserialize)]
pub struct ProbeDraftBody {
    #[serde(default)]
    pub id: Option<uuid::Uuid>,
    pub target: target_repo::TargetInput,
}

/// POST /api/admin/dr-backup/targets/:id/take-ownership — explicit takeover
/// (siyuan #15 §3: "last configured instance wins", but only on request).
pub async fn take_ownership(
    State(state): State<AppState>,
    auth: AuthUser,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    let row = target_repo::get_target(&state.db, id).await?;
    let conn = row.connect().await?;
    let previous = crate::target::owner::read_owner(&conn).await?;
    crate::target::owner::claim(&conn, true).await?;
    let now = crate::target::owner::current_owner();
    Ok(ApiOk::ok(json!({
        "instance_id": now.instance_id,
        "previous_instance": previous.map(|p| p.instance_id),
    })))
}

/// POST /api/admin/dr-backup/targets/:id/empty — clear all objects.
pub async fn empty_target(
    State(state): State<AppState>,
    auth: AuthUser,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    let deleted = target_repo::empty_target(&state.db, id).await?;
    Ok(ApiOk::ok(json!({ "deleted_objects": deleted })))
}

#[derive(Debug, serde::Deserialize)]
pub struct SyncTargetBody {
    #[serde(default)]
    pub user_id: Option<String>,
}

/// POST /api/admin/dr-backup/targets/:id/sync — manual fan-out now.
/// Without user_id, syncs every user assigned to the target.
pub async fn sync_target(
    State(state): State<AppState>,
    auth: AuthUser,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
    body: Option<axum::Json<SyncTargetBody>>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    let storage = state
        .storage
        .clone()
        .ok_or(ApiError::NotFound("storage not configured".into()))?;
    let row = target_repo::get_target(&state.db, id).await?;
    let users: Vec<i64> = match body.and_then(|b| b.0.user_id) {
        Some(uid) => uid
            .parse::<i64>()
            .map(|v| vec![v])
            .map_err(|_| ApiError::BadRequest("user_id must be numeric".into()))?,
        None => row.users.clone(),
    };
    let mut results = Vec::new();
    for uid in users {
        for r in fanout::fanout_user(&state.db, &storage, uid).await {
            results.push(json!(r));
        }
    }
    Ok(ApiOk::ok(json!(results)))
}

/// POST /api/admin/dr-backup/targets/:id/fetch — pull a user's objects from
/// the target back into the local bucket (prerequisite for restore).
pub async fn fetch_target(
    State(state): State<AppState>,
    auth: AuthUser,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
    axum::Json(body): axum::Json<SyncTargetBody>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    let storage = state
        .storage
        .clone()
        .ok_or(ApiError::NotFound("storage not configured".into()))?;
    let uid = body
        .user_id
        .as_deref()
        .ok_or(ApiError::BadRequest("user_id is required".into()))?
        .parse::<i64>()
        .map_err(|_| ApiError::BadRequest("user_id must be numeric".into()))?;
    let fetched = fanout::fetch_user_from_target(&state.db, &storage, id, uid).await?;
    Ok(ApiOk::ok(json!({ "fetched": fetched })))
}

/// GET /api/admin/dr-backup/backup-users — users that own data (candidates
/// for target assignment). Admin only; ids are snowflake strings on the wire.
pub async fn list_backup_users(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    let rows: Vec<(i64, String, Option<String>)> = sqlx::query_as(
        "SELECT DISTINCT u.id, u.username, u.email FROM users u \
         JOIN nodes n ON n.user_id = u.id ORDER BY u.username",
    )
    .fetch_all(&state.db)
    .await
    .map_err(ApiError::from)?;
    let out: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|(id, username, email)| {
            json!({
                "id": id.to_string(),
                "username": username,
                "email": email.unwrap_or_default(),
            })
        })
        .collect();
    Ok(ApiOk::ok(json!(out)))
}

/// GET /api/admin/dr-backup/status — lock state for the frontend read-only
/// banner (also reachable while locked, see the middleware exemptions).
pub async fn lock_status(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<ApiOk<crate::locks::LockSnapshot>, ApiError> {
    require_admin(&auth)?;
    Ok(ApiOk::ok(state.locks.snapshot().await))
}

/// GET /api/admin/dr-backup/targets/:id/export/:snapshot_id — download a
/// portable zip of one snapshot stored on a target (fork §6 "export zip").
pub async fn export_target_snapshot(
    State(state): State<AppState>,
    auth: AuthUser,
    axum::extract::Path((target_id, snapshot_id)): axum::extract::Path<(uuid::Uuid, String)>,
) -> Result<axum::response::Response, ApiError> {
    require_admin(&auth)?;
    if snapshot_id.is_empty() || !snapshot_id.chars().all(|c| c.is_ascii_digit()) {
        return Err(ApiError::BadRequest("invalid snapshot id".into()));
    }
    let row = target_repo::get_target(&state.db, target_id).await?;
    let conn = row.connect().await?;
    let manifest_key = format!("{snapshot_id}.json");
    let objects = conn.list(conn.list_prefix(&row.prefix)).await?;
    let manifest_obj = objects
        .iter()
        .find(|(k, _, _)| k.ends_with(&format!("/manifests/{manifest_key}")))
        .map(|(k, _, _)| k.clone())
        .ok_or(ApiError::NotFound("snapshot not found on target".into()))?;
    let raw = conn.get(&manifest_obj).await?;
    let manifest: crate::backup::BackupManifest = serde_json::from_slice(&raw)
        .map_err(|e| ApiError::BadRequest(format!("manifest unreadable: {e}")))?;
    // Assemble the portable set straight from target objects (no local copy).
    use axum::response::IntoResponse;
    let mut archive = crate::backup::SimpleArchive::new();
    let meta = crate::restore::PortableMeta {
        format: crate::restore::PORTABLE_FORMAT.into(),
        version: crate::restore::PORTABLE_VERSION.into(),
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
    archive.add("manifest.json", &raw);
    let mut seen = std::collections::HashSet::new();
    let mut hashes: Vec<String> = manifest.entries.iter().map(|e| e.sha256.clone()).collect();
    hashes.extend(manifest.files.iter().map(|f| f.sha256.clone()));
    for sha in hashes {
        if !seen.insert(sha.clone()) {
            continue;
        }
        let dir = if manifest.encryption.is_some() {
            "blobs-e2e"
        } else {
            "blobs"
        };
        let key = objects
            .iter()
            .find(|(k, _, _)| k.ends_with(&format!("/{dir}/{sha}")))
            .map(|(k, _, _)| k.clone())
            .ok_or(ApiError::NotFound(format!("blob {sha} missing on target")))?;
        archive.add_stored(format!("blobs/{sha}"), &conn.get(&key).await?);
    }
    let bytes = archive
        .finish()
        .map_err(|e| ApiError::BadRequest(format!("export assembly failed: {e}")))?;
    Ok((
        axum::http::StatusCode::OK,
        [
            (axum::http::header::CONTENT_TYPE, "application/zip"),
            (
                axum::http::header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"pergame-snapshot-{snapshot_id}.zip\"").as_str(),
            ),
        ],
        bytes,
    )
        .into_response())
}
