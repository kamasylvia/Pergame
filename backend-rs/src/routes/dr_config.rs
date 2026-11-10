//! Structured dr-backup configuration: JSON ⇄ bash-sourceable conf
//! (spec backup-restore.md §10, form simplification).
//!
//! The sidecar keeps sourcing a plain bash file — this module only translates
//! between that file and the JSON the admin UI edits, so the deployment
//! contract is unchanged. Secrets are never returned: every credential field
//! serializes as `***` and the writer falls back to the previous value unless a
//! new one was typed (same round-trip rule the raw text editor used).

use serde::{Deserialize, Serialize};

pub const MASK: &str = "***";
/// Default retention when the conf has no RETAIN_DAYS (dr-backup.sh parity).
const DEFAULT_RETAIN_DAYS: u32 = 14;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PgSource {
    pub name: String,
    pub host: String,
    pub port: u32,
    pub database: String,
    pub username: String,
    #[serde(default = "mask_default")]
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct S3Destination {
    pub name: String,
    pub endpoint: String,
    pub bucket: String,
    pub prefix: String,
    #[serde(default = "mask_default")]
    pub access_key: String,
    #[serde(default = "mask_default")]
    pub secret_key: String,
    pub secure: bool,
    pub region: String,
    /// rclone provider ("Other" for MinIO/RustFS/Garage, "AWS" for AWS).
    pub provider: String,
    /// Per-destination retention: prune dumps older than this many days.
    /// 0 = inherit the global RETAIN_DAYS. (Siyuan parity: per-target
    /// retention, batch 2026-11-09.)
    #[serde(default)]
    pub retain_days: u32,
    /// Per-destination retention: always keep at least this many newest
    /// snapshots. 0 = disabled (age pruning only).
    #[serde(default)]
    pub keep_last: u32,
    /// 5-field cron ("30 3 * * *") for scheduled backups of this destination.
    /// Empty = manual only. Parsed with croner's standard-crontab rules.
    #[serde(default)]
    pub cron: String,
    /// Optional rclone-crypt passphrase; empty = store dumps in plaintext.
    /// Empty is a meaningful value ("off"), so unlike the credential fields it
    /// is only masked when actually set.
    #[serde(default)]
    pub crypt_pass: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WebdavDestination {
    pub name: String,
    pub url: String,
    pub username: String,
    #[serde(default = "mask_default")]
    pub password: String,
    pub prefix: String,
    #[serde(default)]
    pub retain_days: u32,
    #[serde(default)]
    pub keep_last: u32,
    #[serde(default)]
    pub cron: String,
    /// See S3Destination::crypt_pass: empty = off, masked only when set.
    #[serde(default)]
    pub crypt_pass: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DrConfig {
    #[serde(default)]
    pub sources: Vec<PgSource>,
    #[serde(default)]
    pub s3_destinations: Vec<S3Destination>,
    #[serde(default)]
    pub webdav_destinations: Vec<WebdavDestination>,
    pub retain_days: u32,
    pub verify_upload: bool,
}

fn mask_default() -> String {
    MASK.to_string()
}

impl Default for DrConfig {
    fn default() -> Self {
        Self {
            sources: Vec::new(),
            s3_destinations: Vec::new(),
            webdav_destinations: Vec::new(),
            retain_days: DEFAULT_RETAIN_DAYS,
            verify_upload: true,
        }
    }
}

/// Rejects field values that would corrupt the pipe-delimited record format or
/// break the bash array literal (dr-backup.conf.example: "no field may contain
/// |").
fn check_field(what: &str, value: &str) -> Result<(), String> {
    if value.contains('|') {
        return Err(format!("{what} must not contain '|'"));
    }
    if value.contains('"') || value.contains('\n') || value.contains('\r') {
        return Err(format!("{what} must not contain quotes or newlines"));
    }
    Ok(())
}

/// Per-destination retention knob bounds (generous caps; 0 = inherit/off).
fn check_knobs(name: &str, retain_days: u32, keep_last: u32) -> Result<(), String> {
    if retain_days > 3650 {
        return Err(format!("destination '{name}': retain_days must be ≤ 3650"));
    }
    if keep_last > 1000 {
        return Err(format!("destination '{name}': keep_last must be ≤ 1000"));
    }
    Ok(())
}

/// Cron expressions are validated at save time (Siyuan parity: the scheduler
/// skips invalid expressions at runtime, but the UI should catch typos here).
fn check_cron(name: &str, expr: &str) -> Result<(), String> {
    if expr.trim().is_empty() {
        return Ok(());
    }
    parse_cron(expr)
        .map(|_| ())
        .map_err(|e| format!("destination '{name}': invalid cron expression '{expr}': {e}"))
}

/// Duplicate-configuration gate (Siyuan deletecheck parity, simplified): two
/// records pointing at the same physical store location (type + endpoint/URL +
/// bucket + prefix) would silently double-write and make the deletion gate
/// ambiguous, so they are rejected at save time.
fn check_duplicates(cfg: &DrConfig) -> Result<(), String> {
    let mut seen: Vec<(String, String)> = Vec::new();
    for d in &cfg.s3_destinations {
        let fp = format!(
            "s3|{}|{}|{}",
            d.endpoint.trim().to_lowercase(),
            d.bucket.trim(),
            d.prefix.trim()
        );
        let label = format!("S3 '{}'", d.name);
        check_seen(&mut seen, fp, &label)?;
    }
    for d in &cfg.webdav_destinations {
        let fp = format!(
            "webdav|{}|{}",
            d.url.trim().to_lowercase(),
            d.prefix.trim()
        );
        let label = format!("WebDAV '{}'", d.name);
        check_seen(&mut seen, fp, &label)?;
    }
    Ok(())
}

fn check_seen(seen: &mut Vec<(String, String)>, fp: String, label: &str) -> Result<(), String> {
    if let Some((_, first)) = seen.iter().find(|(s, _)| s == &fp) {
        return Err(format!(
            "{label} duplicates {first} (same store location); remove one of them"
        ));
    }
    seen.push((fp, label.to_string()));
    Ok(())
}

/// Parses a 5-field standard crontab expression ("30 3 * * *") — croner's
/// default mode (seconds disallowed, Vixie OR semantics for DOM/DOW).
pub fn parse_cron(expr: &str) -> Result<croner::Cron, String> {
    croner::Cron::new(expr).parse().map_err(|e| e.to_string())
}

impl DrConfig {
    pub fn validate(&self) -> Result<(), String> {
        for s in &self.sources {
            if s.name.trim().is_empty() {
                return Err("source name is required".into());
            }
            if s.host.trim().is_empty() {
                return Err(format!("source '{}': host is required", s.name));
            }
            if s.database.trim().is_empty() {
                return Err(format!("source '{}': database is required", s.name));
            }
            if s.username.trim().is_empty() {
                return Err(format!("source '{}': username is required", s.name));
            }
            check_field("source name", &s.name)?;
            check_field("host", &s.host)?;
            check_field("database", &s.database)?;
            check_field("username", &s.username)?;
            check_field("password", &s.password)?;
        }
        for d in &self.s3_destinations {
            if d.name.trim().is_empty() {
                return Err("S3 destination name is required".into());
            }
            if d.endpoint.trim().is_empty() {
                return Err(format!("S3 '{}': endpoint is required", d.name));
            }
            if d.bucket.trim().is_empty() {
                return Err(format!("S3 '{}': bucket is required", d.name));
            }
            check_field("S3 name", &d.name)?;
            check_field("endpoint", &d.endpoint)?;
            check_field("bucket", &d.bucket)?;
            check_field("prefix", &d.prefix)?;
            check_field("access key", &d.access_key)?;
            check_field("secret key", &d.secret_key)?;
            check_field("region", &d.region)?;
            check_field("provider", &d.provider)?;
            check_field("crypt passphrase", &d.crypt_pass)?;
            check_knobs(&d.name, d.retain_days, d.keep_last)?;
            check_cron(&d.name, &d.cron)?;
        }
        for d in &self.webdav_destinations {
            if d.name.trim().is_empty() {
                return Err("WebDAV destination name is required".into());
            }
            if d.url.trim().is_empty() {
                return Err(format!("WebDAV '{}': URL is required", d.name));
            }
            check_field("WebDAV name", &d.name)?;
            check_field("URL", &d.url)?;
            check_field("username", &d.username)?;
            check_field("password", &d.password)?;
            check_field("prefix", &d.prefix)?;
            check_field("crypt passphrase", &d.crypt_pass)?;
            check_knobs(&d.name, d.retain_days, d.keep_last)?;
            check_cron(&d.name, &d.cron)?;
        }
        check_duplicates(self)?;
        Ok(())
    }

    /// Renders the bash conf the sidecar sources. Comments are minimal since
    /// the UI is the primary editor; the format stays identical to
    /// dr-backup.conf.example.
    pub fn to_conf(&self) -> String {
        let mut out = String::from(
            "# dr-backup configuration — managed by the Pergame admin UI.\n\
             # Format: bash-sourceable; records are pipe-delimited.\n",
        );

        out.push_str("\nSOURCES=(\n");
        for s in &self.sources {
            out.push_str(&format!(
                "  \"pg|{}|{}|{}|{}|{}|{}\"\n",
                s.name, s.host, s.port, s.username, s.password, s.database
            ));
        }
        out.push_str(")\n");

        out.push_str("\nS3_DESTINATIONS=(\n");
        for d in &self.s3_destinations {
            out.push_str(&format!(
                "  \"s3|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}\"\n",
                d.name,
                d.endpoint,
                d.bucket,
                d.prefix,
                d.access_key,
                d.secret_key,
                if d.secure { "true" } else { "false" },
                d.region,
                d.provider,
                d.retain_days,
                d.keep_last,
                d.cron.trim(),
                d.crypt_pass
            ));
        }
        out.push_str(")\n");

        out.push_str("\nWEBDAV_DESTINATIONS=(\n");
        for d in &self.webdav_destinations {
            out.push_str(&format!(
                "  \"webdav|{}|{}|{}|{}|{}|{}|{}|{}|{}\"\n",
                d.name,
                d.url,
                d.username,
                d.password,
                d.prefix,
                d.retain_days,
                d.keep_last,
                d.cron.trim(),
                d.crypt_pass
            ));
        }
        out.push_str(")\n");

        out.push_str(&format!(
            "\nRETAIN_DAYS={}\nVERIFY_UPLOAD={}\n",
            self.retain_days,
            if self.verify_upload { 1 } else { 0 }
        ));
        out
    }

    /// Parses a conf file. Unknown lines (comments, blanks, future settings)
    /// are ignored; a malformed record is an error so a typo surfaces in the UI
    /// instead of silently dropping a destination.
    pub fn from_conf(content: &str) -> Result<Self, String> {
        let mut cfg = DrConfig::default();
        // Track which array we are inside; bash arrays span multiple lines.
        let mut in_array: Option<&str> = None;

        for raw in content.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(var) = line.strip_suffix("=(") {
                in_array = Some(var);
                continue;
            }
            if line == ")" {
                in_array = None;
                continue;
            }
            if let Some(value) = line.strip_prefix("RETAIN_DAYS=") {
                cfg.retain_days = value.trim().parse().unwrap_or(DEFAULT_RETAIN_DAYS);
                continue;
            }
            if let Some(value) = line.strip_prefix("VERIFY_UPLOAD=") {
                cfg.verify_upload = value.trim() != "0";
                continue;
            }
            if let Some(array) = in_array {
                let record = line
                    .trim_end_matches(',')
                    .trim()
                    .trim_matches('"')
                    .to_string();
                cfg.push_record(array, &record)?;
            }
        }
        // Keep secrets masked: the raw file holds real values, but this struct
        // is only ever handed to the UI, which must not receive them.
        cfg.mask_all();
        Ok(cfg)
    }

    fn push_record(&mut self, array: &str, record: &str) -> Result<(), String> {
        let f: Vec<&str> = record.split('|').collect();
        match array {
            "SOURCES" => {
                if f.len() < 7 || f[0] != "pg" {
                    return Err(format!("malformed SOURCES record: {record}"));
                }
                self.sources.push(PgSource {
                    name: f[1].into(),
                    host: f[2].into(),
                    port: f[3].parse().unwrap_or(5432),
                    username: f[4].into(),
                    password: f[5].into(),
                    database: f[6].into(),
                });
            }
            "S3_DESTINATIONS" => {
                if f.len() < 10 || f[0] != "s3" {
                    return Err(format!("malformed S3_DESTINATIONS record: {record}"));
                }
                self.s3_destinations.push(S3Destination {
                    name: f[1].into(),
                    endpoint: f[2].into(),
                    bucket: f[3].into(),
                    prefix: f[4].into(),
                    access_key: f[5].into(),
                    secret_key: f[6].into(),
                    secure: f[7] != "false",
                    region: f[8].into(),
                    provider: f[9].into(),
                    // Optional trailing knobs (Siyuan parity batch): old confs
                    // lack them — absent means inherit/off.
                    retain_days: f.get(10).and_then(|v| v.parse().ok()).unwrap_or(0),
                    keep_last: f.get(11).and_then(|v| v.parse().ok()).unwrap_or(0),
                    cron: f.get(12).copied().unwrap_or("").into(),
                    crypt_pass: f.get(13).copied().unwrap_or("").into(),
                });
            }
            "WEBDAV_DESTINATIONS" => {
                if f.len() < 6 || f[0] != "webdav" {
                    return Err(format!("malformed WEBDAV_DESTINATIONS record: {record}"));
                }
                self.webdav_destinations.push(WebdavDestination {
                    name: f[1].into(),
                    url: f[2].into(),
                    username: f[3].into(),
                    password: f[4].into(),
                    prefix: f[5].into(),
                    retain_days: f.get(6).and_then(|v| v.parse().ok()).unwrap_or(0),
                    keep_last: f.get(7).and_then(|v| v.parse().ok()).unwrap_or(0),
                    cron: f.get(8).copied().unwrap_or("").into(),
                    crypt_pass: f.get(9).copied().unwrap_or("").into(),
                });
            }
            other => return Err(format!("unexpected array: {other}")),
        }
        Ok(())
    }

    fn mask_all(&mut self) {
        for s in &mut self.sources {
            s.password = MASK.into();
        }
        for d in &mut self.s3_destinations {
            d.access_key = MASK.into();
            d.secret_key = MASK.into();
            // Empty crypt = "encryption off" — a setting, not a secret; only
            // a configured passphrase is masked.
            if !d.crypt_pass.is_empty() {
                d.crypt_pass = MASK.into();
            }
        }
        for d in &mut self.webdav_destinations {
            d.password = MASK.into();
            if !d.crypt_pass.is_empty() {
                d.crypt_pass = MASK.into();
            }
        }
    }

    /// Replaces `***` with the matching value from `previous` (matched by
    /// destination name). A new record that still carries `***` is rejected —
    /// otherwise the literal mask would end up in the conf file.
    pub fn fill_secrets(&mut self, previous: &DrConfig) -> Result<(), String> {
        for s in &mut self.sources {
            if let Some(old) = previous.sources.iter().find(|o| o.name == s.name) {
                if s.password == MASK {
                    s.password = old.password.clone();
                }
            } else if s.password == MASK {
                return Err(format!(
                    "source '{}' is new: enter its password (it cannot be ***)",
                    s.name
                ));
            }
        }
        for d in &mut self.s3_destinations {
            if let Some(old) = previous.s3_destinations.iter().find(|o| o.name == d.name) {
                if d.access_key == MASK {
                    d.access_key = old.access_key.clone();
                }
                if d.secret_key == MASK {
                    d.secret_key = old.secret_key.clone();
                }
                if d.crypt_pass == MASK {
                    d.crypt_pass = old.crypt_pass.clone();
                }
            } else if d.access_key == MASK || d.secret_key == MASK || d.crypt_pass == MASK {
                return Err(format!(
                    "S3 destination '{}' is new: enter its credentials (they cannot be ***)",
                    d.name
                ));
            }
        }
        for d in &mut self.webdav_destinations {
            if let Some(old) = previous
                .webdav_destinations
                .iter()
                .find(|o| o.name == d.name)
            {
                if d.password == MASK {
                    d.password = old.password.clone();
                }
                if d.crypt_pass == MASK {
                    d.crypt_pass = old.crypt_pass.clone();
                }
            } else if d.password == MASK || d.crypt_pass == MASK {
                return Err(format!(
                    "WebDAV destination '{}' is new: enter its password (it cannot be ***)",
                    d.name
                ));
            }
        }
        Ok(())
    }
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
RETAIN_DAYS=14
VERIFY_UPLOAD=1"#;

    #[test]
    fn parses_every_section() {
        let cfg = DrConfig::from_conf(SAMPLE).expect("parse");
        assert_eq!(cfg.sources.len(), 1);
        assert_eq!(cfg.sources[0].name, "pergame");
        assert_eq!(cfg.sources[0].port, 5432);
        assert_eq!(cfg.s3_destinations.len(), 1);
        assert_eq!(cfg.s3_destinations[0].provider, "Other");
        assert!(cfg.s3_destinations[0].secure);
        assert_eq!(cfg.webdav_destinations.len(), 1);
        assert_eq!(cfg.retain_days, 14);
        assert!(cfg.verify_upload);
    }

    #[test]
    fn parsed_config_hides_secrets() {
        let cfg = DrConfig::from_conf(SAMPLE).expect("parse");
        assert_eq!(cfg.sources[0].password, MASK);
        assert_eq!(cfg.s3_destinations[0].access_key, MASK);
        assert_eq!(cfg.s3_destinations[0].secret_key, MASK);
        assert_eq!(cfg.webdav_destinations[0].password, MASK);
    }

    #[test]
    fn roundtrip_preserves_values() {
        // The form sends masked secrets; the endpoint rehydrates them from the
        // raw file first (raw_secret lives in admin_dr). Mirror that here.
        let mut cfg = DrConfig::from_conf(SAMPLE).expect("parse");
        cfg.sources[0].password = "secret-pass".into();
        cfg.s3_destinations[0].access_key = "AK".into();
        cfg.s3_destinations[0].secret_key = "SK".into();
        cfg.webdav_destinations[0].password = "dav-pass".into();

        let rendered = cfg.to_conf();
        assert!(
            rendered.contains("pg|pergame|localhost|5432|backup_user|secret-pass|pergame"),
            "pg source record: {rendered}"
        );
        // Rendered records carry the full trailing knob set (0 = inherit/off,
        // empty cron/crypt), so old and new scripts can both source them.
        assert!(
            rendered.contains(
                "s3|primary|https://s3.example.com:9000|pergame-backups|pg|AK|SK|true|us-east-1|Other|0|0||"
            ),
            "s3 record: {rendered}"
        );
        assert!(
            rendered.contains("webdav|nas|https://nas/dav/|backup_user|dav-pass|pergame/pg|0|0||"),
            "webdav record: {rendered}"
        );
        assert!(rendered.contains("RETAIN_DAYS=14"));

        // Parsing the rendered file back must yield the same structure.
        let reparsed = DrConfig::from_conf(&rendered).expect("reparse");
        assert_eq!(reparsed.sources.len(), 1);
        assert_eq!(reparsed.sources[0].name, "pergame");
        assert_eq!(reparsed.s3_destinations[0].bucket, "pergame-backups");
        assert_eq!(reparsed.retain_days, 14);
        assert!(reparsed.verify_upload);
    }

    #[test]
    fn new_record_with_mask_is_rejected() {
        let previous = DrConfig::from_conf(SAMPLE).expect("parse");
        let mut cfg = DrConfig::default();
        cfg.sources.push(PgSource {
            name: "brand-new".into(),
            host: "h".into(),
            port: 5432,
            database: "d".into(),
            username: "u".into(),
            password: MASK.into(),
        });
        assert!(cfg.fill_secrets(&previous).is_err());
    }

    #[test]
    fn rejects_pipe_in_fields() {
        let mut cfg = DrConfig::default();
        cfg.sources.push(PgSource {
            name: "bad|name".into(),
            host: "h".into(),
            port: 5432,
            database: "d".into(),
            username: "u".into(),
            password: "p".into(),
        });
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn rejects_quote_in_fields() {
        let mut cfg = DrConfig::default();
        cfg.sources.push(PgSource {
            name: "bad\"name".into(),
            host: "h".into(),
            port: 5432,
            database: "d".into(),
            username: "u".into(),
            password: "p".into(),
        });
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn defaults_when_knobs_absent() {
        let cfg = DrConfig::from_conf("SOURCES=(\n)\n").expect("parse");
        assert_eq!(cfg.retain_days, DEFAULT_RETAIN_DAYS);
        assert!(cfg.verify_upload);
        assert!(cfg.sources.is_empty());
    }

    const FULL_RECORDS: &str = r#"SOURCES=(
  "pg|pergame|localhost|5432|u|p|pergame"
)
S3_DESTINATIONS=(
  "s3|primary|https://s3.example.com|bk|pg|AK|SK|true|us-east-1|Other|30|5|0 3 * * *|cryptpass"
)
WEBDAV_DESTINATIONS=(
  "webdav|nas|https://nas/dav/|u|p|pergame/pg|7|3|15 4 * * 0|"
)"#;

    #[test]
    fn parses_per_destination_knobs() {
        let cfg = DrConfig::from_conf(FULL_RECORDS).expect("parse");
        let s3 = &cfg.s3_destinations[0];
        assert_eq!(s3.retain_days, 30);
        assert_eq!(s3.keep_last, 5);
        assert_eq!(s3.cron, "0 3 * * *");
        assert_eq!(s3.crypt_pass, MASK, "crypt passphrase must be masked");
        let dav = &cfg.webdav_destinations[0];
        assert_eq!(dav.retain_days, 7);
        assert_eq!(dav.keep_last, 3);
        assert_eq!(dav.cron, "15 4 * * 0");
        assert_eq!(dav.crypt_pass, "", "empty crypt stays empty after masking");
    }

    #[test]
    fn old_records_get_zero_defaults() {
        let cfg = DrConfig::from_conf(SAMPLE).expect("parse");
        let s3 = &cfg.s3_destinations[0];
        assert_eq!(s3.retain_days, 0, "absent retain = inherit global");
        assert_eq!(s3.keep_last, 0);
        assert_eq!(s3.cron, "");
        assert_eq!(s3.crypt_pass, "");
    }

    #[test]
    fn crypt_pass_fills_from_previous() {
        // Mirror the endpoint: `previous` comes from from_conf (masked), then
        // admin_dr rehydrates real secret values from the raw file with
        // raw_secret before fill_secrets runs.
        let mut previous = DrConfig::from_conf(FULL_RECORDS).expect("parse");
        previous.s3_destinations[0].crypt_pass = "cryptpass".into();
        let mut cfg = DrConfig::from_conf(FULL_RECORDS).expect("parse");
        cfg.s3_destinations[0].bucket = "renamed".into();
        cfg.fill_secrets(&previous).expect("fill");
        assert_eq!(cfg.s3_destinations[0].crypt_pass, "cryptpass");
    }

    #[test]
    fn rejects_invalid_cron() {
        let mut cfg = DrConfig::from_conf(FULL_RECORDS).expect("parse");
        cfg.s3_destinations[0].cron = "not a cron".into();
        let err = cfg.validate().expect_err("invalid cron must fail");
        assert!(err.contains("invalid cron expression"), "{err}");
        cfg.s3_destinations[0].cron = "30 3 * * *".into();
        cfg.validate().expect("valid cron passes");
    }

    #[test]
    fn rejects_duplicate_store_location() {
        let mut cfg = DrConfig::from_conf(FULL_RECORDS).expect("parse");
        let mut dup = cfg.s3_destinations[0].clone();
        dup.name = "second".into();
        cfg.s3_destinations.push(dup);
        let err = cfg.validate().expect_err("duplicate must fail");
        assert!(err.contains("duplicates"), "{err}");
    }

    #[test]
    fn rejects_out_of_range_knobs() {
        let mut cfg = DrConfig::from_conf(FULL_RECORDS).expect("parse");
        cfg.s3_destinations[0].keep_last = 5000;
        assert!(cfg.validate().is_err());
    }
}
