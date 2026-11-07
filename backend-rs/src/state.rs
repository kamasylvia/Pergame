use sqlx::PgPool;
use toasty::Db;

use crate::types::SnowflakeGenerator;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    /// Toasty ORM handle (same database; schema owned by sqlx migrations).
    pub toasty: Db,
    /// Shared Snowflake id generator.
    pub snowflake: std::sync::Arc<SnowflakeGenerator>,
}
