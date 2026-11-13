//! POST /nodes/query — DQL subset execution (issue #5).
//!
//! Request: `{"query": "TABLE name FROM \"Projects\" ..."}`. Scope is the
//! caller's own nodes only (owned rows are trivially authorized); `FROM`
//! folder paths resolve via [`node_repo::resolve_folder_path`]. The planned
//! statement runs under a 5s timeout.

use std::collections::HashMap;
use std::time::Duration;

use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::dql;
use crate::error::ApiError;
use crate::repo::nodes as node_repo;
use crate::response::ApiOk;

/// POST /nodes/query body.
#[derive(Debug, Deserialize)]
pub struct QueryInput {
    #[serde(default)]
    pub query: String,
}

/// POST /nodes/query — parse, plan, execute (own nodes, 5s timeout).
pub async fn query(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<QueryInput>,
) -> Result<impl IntoResponse, ApiError> {
    if input.query.trim().is_empty() {
        return Err(ApiError::BadRequest("query must not be empty".into()));
    }
    if input.query.len() > 4096 {
        return Err(ApiError::BadRequest("query too long (max 4096 chars)".into()));
    }
    let q = dql::parse(&input.query).map_err(ApiError::BadRequest)?;
    let folders = resolve_folders(&state, auth.user_id(), &q).await?;
    let planned = dql::plan(&q, &folders, node_repo::now_ms());
    let rows = tokio::time::timeout(
        Duration::from_secs(5),
        node_repo::dql_query(&state.db, auth.user_id(), &planned),
    )
    .await
    .map_err(|_| ApiError::BadRequest("query timed out".into()))??;
    let columns: Option<Vec<String>> = match &q.output {
        dql::Output::List => None,
        dql::Output::Table(cols) => Some(cols.clone()),
    };
    Ok(ApiOk::ok(json!({
        "columns": columns,
        "rows": rows,
    })))
}

async fn resolve_folders(
    state: &AppState,
    user_id: Uuid,
    q: &dql::Query,
) -> Result<HashMap<String, Option<Uuid>>, ApiError> {
    let mut out = HashMap::new();
    for src in &q.sources {
        if let dql::Source::Folder(path) = src
            && !out.contains_key(path) {
                let id = node_repo::resolve_folder_path(&state.db, user_id, path).await?;
                out.insert(path.clone(), id);
            }
    }
    Ok(out)
}
