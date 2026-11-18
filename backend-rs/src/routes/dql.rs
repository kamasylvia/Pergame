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
    // `viewed_timestamp` lives in node_views, not in the row shape: merge it
    // in so TABLE projections (and any consumer) can display it.
    let mut out_rows: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))
        .collect();
    if !out_rows.is_empty() {
        let ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
        if let Ok(views) = node_repo::viewed_map(&state.db, auth.user_id(), &ids).await {
            for v in &mut out_rows {
                if let Some(id) = v
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|s| Uuid::parse_str(s).ok())
                    .and_then(|id| views.get(&id).copied())
                {
                    v["viewed_timestamp"] = json!(id);
                }
            }
        }
    }
    Ok(ApiOk::ok(json!({
        "columns": columns,
        "rows": out_rows,
    })))
}

async fn resolve_folders(
    state: &AppState,
    user_id: Uuid,
    q: &dql::Query,
) -> Result<HashMap<String, Option<Uuid>>, ApiError> {
    let mut out = HashMap::new();
    for src in &q.sources {
        // Stable `id:` sources skip path resolution entirely (rename/move
        // proof); the planner binds them directly.
        if matches!(src, dql::Source::FolderId(_)) {
            continue;
        }
        if let dql::Source::Folder(path) = src
            && !out.contains_key(path) {
                let id = node_repo::resolve_folder_path(&state.db, user_id, path).await?;
                out.insert(path.clone(), id);
            }
    }
    Ok(out)
}
