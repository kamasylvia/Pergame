//! Database connection pool and startup migrations (sqlx).

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

use crate::config::Config;

/// Connect the sqlx pool (10 conns, 5s acquire timeout).
pub async fn connect(cfg: &Config) -> Result<PgPool, sqlx::Error> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect(&cfg.postgres_url())
        .await?;
    Ok(pool)
}

/// Runs embedded migrations at startup (mirrors Go `app.Migrate`).
pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}
