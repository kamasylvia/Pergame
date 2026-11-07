//! Toasty models mapping onto the hand-managed pg schema (migrations/0001_init.sql).
//!
//! Design (issue #1): schema ownership stays with `sqlx migrate` — Toasty
//! never calls `push_schema`; models are declared to match the existing
//! tables. Ids are application-assigned Snowflakes (`#[key]` without
//! `#[auto]`), stored as BIGINT.

use serde::{Deserialize, Serialize};
use toasty::Model;

#[derive(Debug, Model, Serialize, Deserialize)]
#[table = "users"]
pub struct User {
    /// Application-assigned Snowflake (no `#[auto]` — not a DB sequence).
    #[key]
    #[column("id")]
    pub id: i64,

    #[column("username")]
    #[unique]
    pub username: String,

    #[column("firstname")]
    pub firstname: Option<String>,

    #[column("lastname")]
    pub lastname: Option<String>,

    /// 1 = user, 2 = admin.
    #[column("role")]
    pub role: i32,

    /// 0 = regular, 1 = supervised.
    #[column("type")]
    pub user_type: i32,

    #[column("avatar")]
    pub avatar: Option<String>,

    #[column("email")]
    pub email: Option<String>,

    /// bcrypt hash ($2a$/$2b$); null for OIDC-only accounts.
    /// Never serialized (Go strips it before responding).
    #[column("password")]
    #[serde(skip_serializing)]
    pub password: Option<String>,

    #[column("totp_secret")]
    #[serde(skip_serializing)]
    pub totp_secret: Option<String>,

    #[column("totp_enabled")]
    pub totp_enabled: bool,

    #[column("totp_forced")]
    pub totp_forced: bool,

    #[column("created_timestamp")]
    pub created_timestamp: i64,

    #[column("updated_timestamp")]
    pub updated_timestamp: i64,
}

#[derive(Debug, Model)]
#[table = "sessions"]
pub struct Session {
    #[key]
    #[column("id")]
    pub id: i64,

    #[index]
    #[column("user_id")]
    pub user_id: i64,

    #[unique]
    #[column("refresh_token")]
    pub refresh_token: String,

    #[column("expire_token")]
    pub expire_token: i64,

    #[column("last_refresh_timestamp")]
    pub last_refresh_timestamp: i64,

    /// 0 = inactive, 1 = active (Go parity: int, not bool).
    #[column("active")]
    pub active: i32,

    #[column("ip_addr")]
    pub ip_addr: Option<String>,

    #[column("user_agent")]
    pub user_agent: Option<String>,

    #[column("location")]
    pub location: Option<String>,

    /// Column name `type` is a Rust keyword; mapped via #[column].
    #[column("type")]
    pub session_type: Option<String>,

    #[column("login_timestamp")]
    pub login_timestamp: i64,

    #[column("logout_timestamp")]
    pub logout_timestamp: i64,
}

/// TOTP backup/recovery codes (8-digit, bcrypt-hashed; Go parity).
#[derive(Debug, Model)]
#[table = "user_totp_recovery_codes"]
pub struct RecoveryCode {
    #[key]
    #[column("id")]
    pub id: i64,

    #[index]
    #[column("user_id")]
    pub user_id: i64,

    /// bcrypt hash of the 8-digit code.
    #[column("code")]
    pub code: String,

    #[column("used")]
    pub used: bool,

    #[column("created_timestamp")]
    pub created_timestamp: i64,
}

/// P1 spike (compile-time gate): exercises the derive-generated query
/// surface against a `Db` handle. Runtime verification follows with a
/// live PostgreSQL.
#[allow(dead_code)]
async fn spike_compile(db: &mut toasty::Db) -> toasty::Result<()> {
    // Read by primary key.
    let _user = User::filter_by_id(42i64).first().exec(db).await?;

    // Filter by a unique column.
    let _by_name = User::filter_by_username("someone").first().exec(db).await?;

    // Create: builder requires every non-default field (ids are app-assigned).
    let _created = User::create()
        .id(1_234_567_890i64)
        .username("someone")
        .role(1)
        .user_type(0)
        .totp_enabled(false)
        .totp_forced(false)
        .created_timestamp(1)
        .updated_timestamp(1)
        .exec(db)
        .await?;

    // Update via query.
    User::filter_by_id(42i64)
        .update()
        .username("renamed")
        .exec(db)
        .await?;

    // Delete.
    User::filter_by_id(42i64).delete().exec(db).await?;

    // Session lookups used by the auth domain.
    let _session = Session::filter_by_refresh_token("abc").first().exec(db).await?;
    let _by_user = Session::filter_by_user_id(1i64).exec(db).await?;

    // Recovery codes used by the TOTP domain.
    let _codes = RecoveryCode::filter_by_user_id(1i64).exec(db).await?;
    let _rc = RecoveryCode::create()
        .id(1i64)
        .user_id(1i64)
        .code("$2a$10$hash")
        .used(false)
        .created_timestamp(1)
        .exec(db)
        .await?;

    Ok(())
}
