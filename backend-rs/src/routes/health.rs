//! Health and misc API routes.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde_json::json;

use crate::AppState;
use crate::error::ApiResult;

/// GET /api/health — liveness; reports database reachability.
pub async fn health(
    State(state): State<AppState>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    let db_ok = sqlx::query("SELECT 1").execute(&state.db).await.is_ok();
    Ok((
        if db_ok {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        },
        Json(json!({ "status": if db_ok { "ok" } else { "degraded" }, "database": db_ok })),
    ))
}
