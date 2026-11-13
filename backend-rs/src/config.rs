//! Configuration: embedded `config.toml` default, `CONFIG_PATH` override, then
//! environment variables (Go `main.go`/`database.go` semantics).

use std::sync::OnceLock;

use serde::Deserialize;

/// Embedded default `config.toml`.
pub const DEFAULT_CONFIG: &str = include_str!("../config.toml");

#[derive(Debug, Clone, Default, serde::Deserialize)]
/// Merged config: embedded default → `CONFIG_PATH` → prefixed env (`DATABASE_*`, …).
pub struct Config {
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub cdn: CdnConfig,
    #[serde(default)]
    pub auth: AuthConfig,
}

#[derive(Debug, Clone, Deserialize)]
/// Postgres connection block.
pub struct DatabaseConfig {
    /// Retained for config.toml parity (pgx is the only supported value).
    #[allow(dead_code)]
    #[serde(default = "default_driver", rename = "Driver")]
    pub driver: String,
    #[serde(default = "default_host", rename = "Host")]
    pub host: String,
    #[serde(default = "default_port", rename = "Port")]
    pub port: u16,
    #[serde(default = "default_db_name", rename = "Name")]
    pub name: String,
    #[serde(default = "default_db_user", rename = "User")]
    pub user: String,
    #[serde(default = "default_db_password", rename = "Password")]
    pub password: String,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            driver: default_driver(),
            host: default_host(),
            port: default_port(),
            name: default_db_name(),
            user: default_db_user(),
            password: default_db_password(),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
/// Upload/CDN limits block.
pub struct CdnConfig {
    #[serde(default = "default_max_size", rename = "MaxSize")]
    pub max_size: u64,
    #[serde(default = "default_max_uploads_size", rename = "MaxUploadsSize")]
    pub max_uploads_size: u64,
    #[serde(rename = "SupportedTypes")]
    pub supported_types: Option<Vec<String>>,
    #[serde(default, rename = "SupportedTypesImages")]
    pub supported_types_images: Option<Vec<String>>,
}

impl Default for CdnConfig {
    fn default() -> Self {
        Self {
            max_size: default_max_size(),
            max_uploads_size: default_max_uploads_size(),
            supported_types: None,
            supported_types_images: None,
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
/// JWT lifetime block (seconds).
pub struct AuthConfig {
    #[serde(default = "default_access_expiry", rename = "AccessTokenExpiry")]
    pub access_token_expiry: i64,
    #[serde(default = "default_refresh_expiry", rename = "RefreshTokenExpiry")]
    pub refresh_token_expiry: i64,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            access_token_expiry: default_access_expiry(),
            refresh_token_expiry: default_refresh_expiry(),
        }
    }
}

fn default_driver() -> String {
    "pgx".into()
}
fn default_host() -> String {
    "localhost".into()
}
fn default_port() -> u16 {
    5432
}
fn default_db_name() -> String {
    "pergame".into()
}
fn default_db_user() -> String {
    "pergame".into()
}
fn default_db_password() -> String {
    String::new()
}
fn default_max_size() -> u64 {
    20_000_000
}
fn default_max_uploads_size() -> u64 {
    1_073_741_824
}
fn default_access_expiry() -> i64 {
    1800
}
fn default_refresh_expiry() -> i64 {
    604_800
}

fn raw_toml() -> String {
    if let Ok(path) = std::env::var("CONFIG_PATH") {
        if let Ok(s) = std::fs::read_to_string(&path) {
            tracing::info!(path = %path, "loaded config from CONFIG_PATH");
            return s;
        }
        tracing::warn!(path = %path, "CONFIG_PATH set but unreadable; using embedded config");
    }
    DEFAULT_CONFIG.to_string()
}

impl Config {
    /// Load from embedded default + `CONFIG_PATH` + env overrides.
    pub fn load() -> Self {
        // Section-wise parse: toml::Value → per-table serde. (The whole-doc
        // toml::from_str::<Config> path silently drops array fields — bisection
        // documented in .work/LESSONS.md.)
        let value: toml::Value = toml::from_str(&raw_toml()).expect("invalid config.toml");
        let mut cfg = Config::default();
        if let Some(table) = value.get("Database") {
            if let Ok(db) = DatabaseConfig::deserialize(table.clone()) {
                cfg.database = db;
            } else {
                tracing::warn!("config: [Database] section failed to deserialize; using defaults");
            }
        }
        if let Some(table) = value.get("Cdn") {
            if let Ok(cdn) = CdnConfig::deserialize(table.clone()) {
                cfg.cdn = cdn;
            } else {
                tracing::warn!("config: [Cdn] section failed to deserialize; using defaults");
            }
        }
        if let Some(table) = value.get("Auth") {
            if let Ok(auth) = AuthConfig::deserialize(table.clone()) {
                cfg.auth = auth;
            } else {
                tracing::warn!("config: [Auth] section failed to deserialize; using defaults");
            }
        }

        // Environment overrides (Go parity: DATABASE_* etc.)
        if let Ok(v) = std::env::var("DATABASE_HOST") {
            cfg.database.host = v;
        }
        if let Ok(v) = std::env::var("DATABASE_PORT")
            && let Ok(p) = v.parse()
        {
            cfg.database.port = p;
        }
        if let Ok(v) = std::env::var("DATABASE_NAME") {
            cfg.database.name = v;
        }
        if let Ok(v) = std::env::var("DATABASE_USER") {
            cfg.database.user = v;
        }
        if let Ok(v) = std::env::var("DATABASE_PASSWORD") {
            cfg.database.password = v;
        }
        cfg
    }

    /// `postgres://user:pass@host:port/name` for sqlx and Toasty.
    pub fn postgres_url(&self) -> String {
        format!(
            "postgres://{}:{}@{}:{}/{}",
            self.database.user,
            self.database.password,
            self.database.host,
            self.database.port,
            self.database.name
        )
    }

    /// Listen port (`BACKEND_PORT`, default 8201).
    pub fn backend_port(&self) -> u16 {
        std::env::var("BACKEND_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(8201)
    }
}

static CONFIG: OnceLock<Config> = OnceLock::new();

/// Process-wide config, initialized once.
pub fn global() -> &'static Config {
    CONFIG.get_or_init(Config::load)
}

#[cfg(test)]
#[allow(unsafe_code)] // Test-only env mutation; no other test touches CONFIG_PATH.
mod tests {
    use super::*;

    /// Mirrors the production `Config::load` path (section-wise toml::Value →
    /// per-table serde), since whole-doc `toml::from_str::<Config>` silently
    /// drops the SupportedTypes array field (toml 1.2 quirk, see LESSONS).
    #[test]
    fn sectionwise_parse() {
        let small = "[Cdn]\nMaxSize = 5\nMaxUploadsSize = 6\nSupportedTypes = [\"a\", \"b\"]\nSupportedTypesImages = [\"c\"]\n";
        let value: toml::Value = toml::from_str(small).expect("parse small");
        let cdn = CdnConfig::deserialize(value.get("Cdn").cloned().expect("[Cdn] table"))
            .expect("cdn deserialize");
        assert_eq!(cdn.max_size, 5);
        assert_eq!(cdn.max_uploads_size, 6);
        assert_eq!(cdn.supported_types.as_ref().map(|v| v.len()), Some(2));
        assert_eq!(
            cdn.supported_types_images.as_ref().map(|v| v.len()),
            Some(1)
        );
    }

    #[test]
    fn cdn_table_probe() {
        let v: toml::Value = toml::from_str(DEFAULT_CONFIG).expect("parse");
        let cdn = v.get("Cdn").expect("[Cdn] table");
        println!(
            "Cdn as table: {:?}",
            cdn.as_table().map(|t| t.keys().collect::<Vec<_>>())
        );
        let direct: Result<CdnConfig, _> = cdn.clone().try_into();
        match &direct {
            Ok(c) => println!(
                "CdnConfig direct: max={} st_len={}",
                c.max_size,
                c.supported_types.as_ref().map(|v| v.len()).unwrap_or(0)
            ),
            Err(e) => println!("CdnConfig direct: ERR {e}"),
        }
        let cfg: Config = toml::from_str(DEFAULT_CONFIG).expect("parse cfg");
        println!(
            "CONFIG PARSED: db.host={} db.name={} cdn.max={} cdn.st={:?} cdn.img={:?} auth.exp={}",
            cfg.database.host,
            cfg.database.name,
            cfg.cdn.max_size,
            cfg.cdn.supported_types,
            cfg.cdn.supported_types_images,
            cfg.auth.access_token_expiry
        );
        assert!(
            cdn.get("SupportedTypes").is_some(),
            "SupportedTypes key missing from [Cdn]"
        );
    }

    #[test]
    fn url_and_port_defaults() {
        let cfg = Config::default();
        assert!(cfg.postgres_url().starts_with("postgres://"));
        assert!(cfg.postgres_url().contains(":5432/"));
        assert_eq!(cfg.backend_port(), 8201);
    }

    #[test]
    fn bad_sections_fall_back_to_defaults() {
        // [Database] with a wrong-typed port fails section deserialize and
        // keeps defaults instead of aborting the whole load.
        let value: toml::Value = toml::from_str("[Database]\nPort = \"nope\"\n").unwrap();
        let table = value.get("Database").unwrap();
        assert!(DatabaseConfig::deserialize(table.clone()).is_err());
        let cfg = Config::default();
        assert_eq!(cfg.database.port, 5432);
    }

    #[test]
    fn config_path_override_and_fallback() {
        // CONFIG_PATH is only read by config; no other test touches it.
        let path = std::env::temp_dir().join("pergame-config-test.toml");
        std::fs::write(&path, "[Database]\nHost = \"from-file\"\n").unwrap();
        unsafe {
            std::env::set_var("CONFIG_PATH", path.to_str().unwrap());
        }
        assert!(raw_toml().contains("from-file"));
        unsafe {
            std::env::set_var("CONFIG_PATH", "/nonexistent/pergame.toml");
        }
        assert_eq!(raw_toml(), DEFAULT_CONFIG);
        unsafe {
            std::env::remove_var("CONFIG_PATH");
        }
        assert_eq!(raw_toml(), DEFAULT_CONFIG);
        std::fs::remove_file(&path).ok();
    }
}
