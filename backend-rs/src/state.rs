use sqlx::PgPool;
use toasty::Db;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    /// Toasty ORM handle (same database; schema owned by sqlx migrations).
    pub toasty: Db,
}
