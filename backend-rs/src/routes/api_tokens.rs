//! Per-user MCP API token management (spec `.work/features/mcp-server.md`).
//!
//! Session-authenticated (`AuthUser`, cookie JWT) on purpose: minting a token
//! must not be possible with a token. The plaintext is returned exactly once
//! by `POST /users/@me/api-tokens`; the database only ever holds its sha256.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::mcp;
use crate::response::ApiOk;

/// GET /users/@me/api-tokens — the caller's tokens (never the hash).
pub async fn list_tokens(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<ApiOk<Vec<mcp::ApiTokenView>>, ApiError> {
    let rows = mcp::list_tokens(&state.db, auth.user_id())
        .await
        .map_err(|e| ApiError::Internal(Box::new(std::io::Error::other(e.to_string()))))?;
    Ok(ApiOk::ok(rows.into_iter().map(Into::into).collect()))
}

#[derive(Debug, Deserialize)]
/// POST /users/@me/api-tokens body.
pub struct CreateTokenInput {
    #[serde(default)]
    pub name: String,
}

/// POST /users/@me/api-tokens — mint a token; the plaintext appears once.
pub async fn create_token(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<CreateTokenInput>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let name = input.name.trim().to_string();
    if name.is_empty() || name.chars().count() > 60 {
        return Err(ApiError::BadRequest("name must be 1-60 characters".into()));
    }

    let created = mcp::create_token(&state.db, auth.user_id(), &name)
        .await
        .map_err(|e| ApiError::Internal(Box::new(std::io::Error::other(e.to_string()))))?;

    mcp::audit(
        &state.db,
        Some(auth.user_id()),
        "mcp.token.create",
        &format!("created api token {name}"),
    )
    .await;

    Ok(ApiOk::with_status(
        StatusCode::CREATED,
        json!({
            "id": created.row.id,
            "name": created.row.name,
            "token": created.plaintext,
            "created_timestamp": created.row.created_timestamp,
        }),
    ))
}

/// DELETE /users/@me/api-tokens/:id — revoke one of the caller's tokens.
pub async fn delete_token(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    let id = Uuid::parse_str(&id).map_err(|_| ApiError::BadRequest("invalid parameter".into()))?;
    let deleted = mcp::delete_token(&state.db, auth.user_id(), id)
        .await
        .map_err(|e| ApiError::Internal(Box::new(std::io::Error::other(e.to_string()))))?;
    if !deleted {
        return Err(ApiError::NotFound("token not found".into()));
    }
    mcp::audit(
        &state.db,
        Some(auth.user_id()),
        "mcp.token.revoke",
        &format!("revoked api token {id}"),
    )
    .await;
    Ok(ApiOk::ok(json!({ "id": id, "deleted": true })))
}
