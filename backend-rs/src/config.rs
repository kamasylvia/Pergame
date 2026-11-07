//! Configuration: embedded `config.toml` default, `CONFIG_PATH` override, then
//! environment variables (Go `main.go`/`database.go` semantics).

use std::sync::OnceLock;

use serde::Deserialize;

pub const DEFAULT_CONFIG: &str = include_str!("../config.toml");

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub cdn: CdnConfig,
    #[serde(default)]
    pub auth: AuthConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    #[serde(default = "default_driver")]
    pub driver: String,
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_db_name")]
    pub name: String,
    #[serde(default = "default_db_user")]
    pub user: String,
    #[serde(default = "default_db_password")]
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

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CdnConfig {
    #[serde(default = "default_max_size")]
    pub max_size: u64,
    #[serde(default = "default_max_uploads_size")]
    pub max_uploads_size: u64,
    #[serde(default)]
    pub supported_types: Option<Vec<String>>,
    #[serde(default)]
    pub supported_types_images: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfig {
    #[serde(default = "default_access_expiry")]
    pub access_token_expiry: i64,
    #[serde(default = "default_refresh_expiry")]
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
    "alexandrie".into()
}
fn default_db_user() -> String {
    "alexandrie".into()
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
    pub fn load() -> Self {
        let mut cfg: Config = toml::from_str(&raw_toml()).expect("invalid config.toml");

        // Environment overrides (Go parity: DATABASE_* etc.)
        if let Ok(v) = std::env::var("DATABASE_HOST") {
            cfg.database.host = v;
        }
        if let Ok(v) = std::env::var("DATABASE_PORT")
            && let Ok(p) = v.parse() {
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

    pub fn postgres_url(&self) -> String {
        format!(
            "postgres://{}:{}@{}:{}/{}",
            self.database.user, self.database.password, self.database.host, self.database.port, self.database.name
        )
    }

    pub fn backend_port(&self) -> u16 {
        std::env::var("BACKEND_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(8201)
    }
}

static CONFIG: OnceLock<Config> = OnceLock::new();

pub fn global() -> &'static Config {
    CONFIG.get_or_init(Config::load)
}
