use sqlx::PgPool;
use toasty::Db;

use crate::storage::S3Storage;
use crate::types::SnowflakeGenerator;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    /// Toasty ORM handle (same database; schema owned by sqlx migrations).
    pub toasty: Db,
    /// Shared Snowflake id generator.
    pub snowflake: std::sync::Arc<SnowflakeGenerator>,
    /// S3-compatible storage (None = CDN features disabled).
    pub storage: Option<S3Storage>,
}
