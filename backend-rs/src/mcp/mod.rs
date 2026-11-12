//! MCP server (spec `.work/features/mcp-server.md`): per-user API tokens and
//! the streamable-HTTP service mounted at `/api/mcp`.
//!
//! Shape (checked against rmcp 3.9.0 sources, not guessed):
//! - `StreamableHttpService` + `nest_service("/api/mcp", ...)` is the
//!   official axum integration (`examples/servers/src/counter_streamhttp.rs`).
//! - Bearer auth is a plain axum middleware in front of that service
//!   (`examples/servers/src/simple_auth_streamhttp.rs`). It never looks at the
//!   session cookie: an MCP client authenticates with `Authorization: Bearer
//!   <pgm_…>` or not at all.
//! - The acting user reaches tool handlers through the request extensions:
//!   the transport copies `http::request::Parts` into `RequestContext`
//!   (`transport/streamable_http_server/tower.rs`, "inject request part to
//!   extensions"), so anything the middleware inserts on the request is
//!   readable as `ctx.extensions.get::<Parts>().extensions.get::<McpActor>()`.
//!
//! Authorization is the normal permission layer: every tool acts as the token
//! owner (`Actor`), so MCP is never a privilege-escalation channel.

pub mod server;

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use rmcp::model::{ErrorCode, ErrorData};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiError;

/// Prefix so a leaked token is recognisable in logs and secret scanners.
pub const TOKEN_PREFIX: &str = "pgm_";

/// The acting identity for one MCP request, injected by `bearer_middleware`
/// and consumed by tool handlers.
#[derive(Debug, Clone, Copy)]
pub struct McpActor {
    pub user_id: Uuid,
    pub role: i32,
    pub token_id: Uuid,
}

/// State for the bearer middleware (kept separate from `AppState` so the
/// layer's state type stays independent of the router's).
#[derive(Clone)]
pub struct McpAuthState {
    pub db: PgPool,
}

// ---------------------------------------------------------------- tokens

/// Mints a new token: `(plaintext, sha256_hex)`. The plaintext is returned to
/// the user once and never persisted.
pub fn generate_token() -> (String, String) {
    use base64::Engine as _;
    use rand::RngExt;

    let mut bytes = [0u8; 32];
    rand::rng().fill(&mut bytes);
    let plaintext = format!(
        "{TOKEN_PREFIX}{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    );
    let hash = hash_token(&plaintext);
    (plaintext, hash)
}

/// SHA-256 hex of the presented token — the only form the database holds.
pub fn hash_token(token: &str) -> String {
    use sha2::{Digest, Sha256};

    let mut h = Sha256::new();
    h.update(token.as_bytes());
    hex::encode(h.finalize())
}

#[derive(Debug, sqlx::FromRow)]
pub struct ApiTokenRow {
    pub id: Uuid,
    pub name: String,
    pub created_timestamp: i64,
    pub last_used_timestamp: Option<i64>,
}

/// Token list entry — no hash ever leaves the database.
#[derive(Debug, serde::Serialize)]
pub struct ApiTokenView {
    pub id: Uuid,
    pub name: String,
    pub created_timestamp: i64,
    pub last_used_timestamp: Option<i64>,
}

impl From<ApiTokenRow> for ApiTokenView {
    fn from(r: ApiTokenRow) -> Self {
        Self {
            id: r.id,
            name: r.name,
            created_timestamp: r.created_timestamp,
            last_used_timestamp: r.last_used_timestamp,
        }
    }
}

pub async fn list_tokens(db: &PgPool, user_id: Uuid) -> Result<Vec<ApiTokenRow>, sqlx::Error> {
    sqlx::query_as::<_, ApiTokenRow>(
        "SELECT id, name, created_timestamp, last_used_timestamp
         FROM api_tokens WHERE user_id = $1 ORDER BY created_timestamp DESC",
    )
    .bind(user_id)
    .fetch_all(db)
    .await
}

/// A freshly minted token: the row plus the plaintext that is shown once.
pub struct CreatedToken {
    pub row: ApiTokenRow,
    pub plaintext: String,
}

pub async fn create_token(
    db: &PgPool,
    user_id: Uuid,
    name: &str,
) -> Result<CreatedToken, sqlx::Error> {
    let (plaintext, token_hash) = generate_token();
    let id = crate::types::new_id();
    let now = crate::auth::service::now_ms();
    let row = sqlx::query_as::<_, ApiTokenRow>(
        "INSERT INTO api_tokens (id, user_id, name, token_hash, created_timestamp)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id, name, created_timestamp, last_used_timestamp",
    )
    .bind(id)
    .bind(user_id)
    .bind(name)
    .bind(&token_hash)
    .bind(now)
    .fetch_one(db)
    .await?;
    Ok(CreatedToken { row, plaintext })
}

/// Deletes one of the caller's tokens; `false` when the id is not theirs.
pub async fn delete_token(db: &PgPool, user_id: Uuid, id: Uuid) -> Result<bool, sqlx::Error> {
    let res = sqlx::query("DELETE FROM api_tokens WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(db)
        .await?;
    Ok(res.rows_affected() > 0)
}

/// Resolves a presented token to its owner and user role.
pub async fn resolve_token(db: &PgPool, token: &str) -> Result<Option<McpActor>, sqlx::Error> {
    let hash = hash_token(token);
    let row: Option<(Uuid, Uuid, i32)> = sqlx::query_as(
        "SELECT t.id, t.user_id, u.role
         FROM api_tokens t JOIN users u ON u.id = t.user_id
         WHERE t.token_hash = $1",
    )
    .bind(&hash)
    .fetch_optional(db)
    .await?;
    Ok(row.map(|(token_id, user_id, role)| McpActor {
        user_id,
        role,
        token_id,
    }))
}

/// Write-on-use `last_used_timestamp` (best effort: a failed touch must not
/// fail the request).
pub async fn touch_token(db: &PgPool, token_id: Uuid) {
    let _ = sqlx::query("UPDATE api_tokens SET last_used_timestamp = $1 WHERE id = $2")
        .bind(crate::auth::service::now_ms())
        .bind(token_id)
        .execute(db)
        .await;
}

/// Audit row in `logs` (the table existed but nothing wrote to it yet; MCP
/// token creation/revocation and write tools are the first producers).
pub async fn audit(db: &PgPool, user_id: Option<Uuid>, action: &str, message: &str) {
    let _ = sqlx::query(
        "INSERT INTO logs (id, user_id, action, message, created_timestamp)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(crate::types::new_id())
    .bind(user_id)
    .bind(action)
    .bind(message)
    .bind(crate::auth::service::now_ms())
    .execute(db)
    .await;
}

// ---------------------------------------------------------------- auth

fn bearer_token(headers: &axum::http::HeaderMap) -> Option<&str> {
    let raw = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let token = raw
        .strip_prefix("Bearer ")
        .or_else(|| raw.strip_prefix("bearer "))?;
    let token = token.trim();
    if token.starts_with(TOKEN_PREFIX) && token.len() > TOKEN_PREFIX.len() {
        Some(token)
    } else {
        None
    }
}

/// axum middleware in front of the MCP service: a valid API token or 401.
/// Same error envelope shape as `AuthUser`'s rejection so clients see one
/// contract across the API.
pub async fn bearer_middleware(
    State(state): State<McpAuthState>,
    headers: axum::http::HeaderMap,
    mut req: Request,
    next: Next,
) -> Response {
    let Some(token) = bearer_token(&headers) else {
        return unauthorized();
    };
    match resolve_token(&state.db, token).await {
        Ok(Some(actor)) => {
            touch_token(&state.db, actor.token_id).await;
            req.extensions_mut().insert(actor);
            next.run(req).await
        }
        Ok(None) => unauthorized(),
        Err(e) => {
            tracing::error!(error = %e, "mcp token lookup failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(serde_json::json!({
                    "status": "error",
                    "timestamp": crate::auth::service::now_ms(),
                    "message": "internal error",
                })),
            )
                .into_response()
        }
    }
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        axum::Json(serde_json::json!({
            "status": "error",
            "timestamp": crate::auth::service::now_ms(),
            "message": "Bad access token.",
        })),
    )
        .into_response()
}

// ---------------------------------------------------------------- service

/// Builds the streamable-HTTP MCP service. Each session gets its own handler
/// instance holding a clone of `AppState` (pools are cheap to clone).
pub fn service(
    state: crate::AppState,
) -> rmcp::transport::streamable_http_server::tower::StreamableHttpService<
    server::PergameMcp,
    rmcp::transport::streamable_http_server::session::local::LocalSessionManager,
> {
    rmcp::transport::streamable_http_server::tower::StreamableHttpService::new(
        move || Ok(server::PergameMcp::new(state.clone())),
        Arc::new(
            rmcp::transport::streamable_http_server::session::local::LocalSessionManager::default(),
        ),
        rmcp::transport::StreamableHttpServerConfig::default(),
    )
}

/// `/api/mcp` plus the bearer layer, as a state-free router the main app can
/// merge after `with_state`.
pub fn app(state: crate::AppState) -> axum::Router {
    axum::Router::new()
        .nest_service("/api/mcp", service(state.clone()))
        .layer(axum::middleware::from_fn_with_state(
            McpAuthState {
                db: state.db.clone(),
            },
            bearer_middleware,
        ))
}

// ---------------------------------------------------------------- errors

/// JSON-RPC codes for API failures (spec table). Custom codes are used raw so
/// the mapping stays stable across protocol versions (`ErrorData::resource_not_found`
/// would be rewritten to -32602 for newer peers).
pub fn mcp_error(err: ApiError) -> ErrorData {
    let (code, message) = match &err {
        ApiError::BadRequest(m) => (ErrorCode::INVALID_PARAMS, m.clone()),
        ApiError::Unauthorized(m) => (ErrorCode(-32001), m.clone()),
        ApiError::NotFound(m) => (ErrorCode(-32002), m.clone()),
        ApiError::Forbidden(m) => (ErrorCode(-32003), m.clone()),
        ApiError::Conflict(m) => (ErrorCode(-32009), m.clone()),
        ApiError::Internal(_) => (ErrorCode::INTERNAL_ERROR, "internal error".into()),
    };
    ErrorData::new(code, message, None)
}

/// The workspace read-only window (backup/restore in flight) rejects writes.
/// Reads stay available: they are what an agent needs to explain the lock.
pub fn locked_error() -> ErrorData {
    ErrorData::new(
        ErrorCode(-32023),
        "Workspace is read-only while a backup or restore is running.",
        Some(serde_json::json!({ "reason": "backup_in_progress" })),
    )
}

pub fn actor_of(
    ctx: &rmcp::service::RequestContext<rmcp::RoleServer>,
) -> Result<McpActor, ErrorData> {
    ctx.extensions
        .get::<axum::http::request::Parts>()
        .and_then(|parts| parts.extensions.get::<McpActor>().copied())
        .ok_or_else(|| ErrorData::new(ErrorCode(-32001), "unauthenticated MCP session", None))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_prefixed_and_hash_is_stable() {
        let (plain, hash) = generate_token();
        assert!(plain.starts_with(TOKEN_PREFIX));
        // 32 bytes -> 43 base64url chars, plus the 4-char prefix.
        assert_eq!(plain.len(), TOKEN_PREFIX.len() + 43);
        assert_eq!(hash.len(), 64, "sha256 hex");
        assert_eq!(hash, hash_token(&plain));
        assert_ne!(generate_token().0, plain, "tokens must differ");
    }

    #[test]
    fn api_error_codes_follow_the_spec_table() {
        assert_eq!(
            mcp_error(ApiError::BadRequest("x".into())).code,
            ErrorCode::INVALID_PARAMS
        );
        assert_eq!(
            mcp_error(ApiError::NotFound("x".into())).code,
            ErrorCode(-32002)
        );
        assert_eq!(
            mcp_error(ApiError::Forbidden("x".into())).code,
            ErrorCode(-32003)
        );
        assert_eq!(
            mcp_error(ApiError::Conflict("x".into())).code,
            ErrorCode(-32009)
        );
        assert_eq!(
            mcp_error(ApiError::Unauthorized("x".into())).code,
            ErrorCode(-32001)
        );
        assert_eq!(locked_error().code, ErrorCode(-32023));
        // Internal details never reach the client.
        let internal = ApiError::Internal(Box::new(std::io::Error::other("secret dsn")));
        assert_eq!(mcp_error(internal).message, "internal error");
    }

    #[test]
    fn bearer_parsing_requires_the_prefix() {
        let mut headers = axum::http::HeaderMap::new();
        assert!(bearer_token(&headers).is_none());
        headers.insert(header::AUTHORIZATION, "Bearer nope".parse().unwrap());
        assert!(
            bearer_token(&headers).is_none(),
            "unprefixed tokens rejected"
        );
        let (plain, _) = generate_token();
        headers.insert(
            header::AUTHORIZATION,
            format!("Bearer {plain}").parse().unwrap(),
        );
        assert_eq!(bearer_token(&headers), Some(plain.as_str()));
    }
}
