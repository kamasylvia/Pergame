//! Toasty handle: secondary data layer alongside the sqlx pool (design:
//! Toasty for ORM ergonomics, sqlx as the raw-SQL escape hatch).

use toasty::Db;
use toasty_driver_postgresql::PostgreSQL;

use crate::config::Config;
use crate::models::{RecoveryCode, Session, User};

/// Connects a Toasty `Db` over the same database the sqlx pool uses.
/// Deliberately does NOT touch schema (`push_schema` is never called —
/// migrations are owned by sqlx).
pub async fn connect(cfg: &Config) -> toasty::Result<Db> {
    let driver = PostgreSQL::new(cfg.postgres_url())?;
    Db::builder()
        .models(toasty::models!(User, Session, RecoveryCode))
        .build(driver)
        .await
}
