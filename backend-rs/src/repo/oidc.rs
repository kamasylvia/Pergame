//! OIDC link repository (sqlx; Go repositories/oidc_provider.repository.go
//! parity).

use sqlx::PgPool;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct OidcLinkRow {
    pub id: i64,
    pub user_id: i64,
    pub provider_name: String,
    pub provider_user_id: String,
    pub created_timestamp: i64,
    pub updated_timestamp: i64,
}

pub async fn get_by_user(pool: &PgPool, user_id: i64) -> Result<Vec<OidcLinkRow>, sqlx::Error> {
    sqlx::query_as::<_, OidcLinkRow>(
        "SELECT id, user_id, provider_name, provider_user_id, created_timestamp, updated_timestamp
         FROM user_oidc_providers WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

pub async fn get_by_provider_and_subject(
    pool: &PgPool,
    provider_name: &str,
    provider_user_id: &str,
) -> Result<Option<OidcLinkRow>, sqlx::Error> {
    sqlx::query_as::<_, OidcLinkRow>(
        "SELECT id, user_id, provider_name, provider_user_id, created_timestamp, updated_timestamp
         FROM user_oidc_providers
         WHERE provider_name = $1 AND provider_user_id = $2",
    )
    .bind(provider_name)
    .bind(provider_user_id)
    .fetch_optional(pool)
    .await
}

pub async fn create(
    pool: &PgPool,
    id: i64,
    user_id: i64,
    provider_name: &str,
    provider_user_id: &str,
    now: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO user_oidc_providers (id, user_id, provider_name, provider_user_id, created_timestamp, updated_timestamp)
         VALUES ($1, $2, $3, $4, $5, $5)",
    )
    .bind(id)
    .bind(user_id)
    .bind(provider_name)
    .bind(provider_user_id)
    .bind(now)
    .execute(pool)
    .await
    .map(|_| ())
}

pub async fn delete(pool: &PgPool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM user_oidc_providers WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map(|_| ())
}

/// Go LinkProvider service tail: duplicate-link check handled by the caller.
pub async fn link_provider(
    pool: &PgPool,
    generator: &crate::types::SnowflakeGenerator,
    user_id: i64,
    provider_name: &str,
    provider_user_id: &str,
) -> Result<(), ApiError> {
    if let Some(existing) =
        get_by_provider_and_subject(pool, provider_name, provider_user_id).await?
    {
        if existing.user_id == user_id {
            return Ok(()); // already linked to this user
        }
        return Err(ApiError::BadRequest(
            "this OIDC account is already linked to another user".into(),
        ));
    }
    let now = crate::auth::service::now_ms();
    create(
        pool,
        generator.generate().as_i64(),
        user_id,
        provider_name,
        provider_user_id,
        now,
    )
    .await
    .map_err(ApiError::from)
}

use crate::error::ApiError;

/// Go HasPassword: a password column value exists (login via password possible).
pub async fn has_password(pool: &PgPool, user_id: i64) -> Result<bool, sqlx::Error> {
    let has: Option<bool> = sqlx::query_scalar(
        "SELECT password IS NOT NULL AND password <> '' FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .ok();
    Ok(has.unwrap_or(false))
}
