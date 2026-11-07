//! Toasty models mapping onto the hand-managed pg schema (migrations/0001_init.sql).
//!
//! P1 spike (issue #1 decision gate): schema ownership stays with `sqlx
//! migrate` — Toasty never calls `push_schema`; models are declared to match
//! the existing tables. Ids are application-assigned Snowflakes (`#[key]`
//! without `#[auto]`), stored as BIGINT.

use toasty::Model;

/// P1 spike (compile-time gate): exercises the derive-generated query
/// surface against a `Db` handle. Compiling proves the ORM's generated
/// builders (find_by_id / filter_by_ / create / update / delete) are
/// usable over the hand-managed schema. Runtime verification follows in
/// P2 with a live PostgreSQL.
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

    Ok(())
}

#[derive(Debug, Model)]
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
    #[column("password")]
    pub password: Option<String>,

    #[column("totp_secret")]
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
