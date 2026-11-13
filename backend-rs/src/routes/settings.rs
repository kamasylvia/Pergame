//! /user/settings routes (Go user_settings.controller.go parity) and
//! /stats admin routes (Go stats.controller.go + stats.service.go parity).

use axum::Json;
use axum::extract::State;
use serde::Deserialize;
use serde_json::json;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::repo::settings_stats as repo;
use crate::response::ApiOk;

// ------------------------------------------------------------------ settings

/// GET /user/settings — defaults when the user has none (Go parity).
pub async fn get_settings(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<ApiOk<repo::SettingsRow>, ApiError> {
    let row = repo::get_settings(&state.db, auth.user_id())
        .await
        .map_err(ApiError::from)?
        .unwrap_or(repo::SettingsRow {
            user_id: auth.user_id(),
            general: json!({}),
            editor: json!({}),
            advanced: json!({}),
        });
    Ok(ApiOk::ok(row))
}

#[derive(Debug, Deserialize)]
/// PUT /user/settings body (general/editor/advanced blobs).
pub struct SaveSettingsBody {
    #[serde(default)]
    pub general: serde_json::Value,
    #[serde(default)]
    pub editor: serde_json::Value,
    #[serde(default)]
    pub advanced: serde_json::Value,
}

/// PUT /user/settings — upsert, returns the stored settings.
pub async fn save_settings(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<SaveSettingsBody>,
) -> Result<ApiOk<repo::SettingsRow>, ApiError> {
    let general = if body.general.is_null() {
        json!({})
    } else {
        body.general.clone()
    };
    let editor = if body.editor.is_null() {
        json!({})
    } else {
        body.editor.clone()
    };
    let advanced = if body.advanced.is_null() {
        json!({})
    } else {
        body.advanced.clone()
    };

    repo::upsert_settings(&state.db, auth.user_id(), &general, &editor, &advanced)
        .await
        .map_err(ApiError::from)?;

    let row = repo::get_settings(&state.db, auth.user_id())
        .await
        .map_err(ApiError::from)?
        .ok_or(ApiError::Internal(Box::new(std::io::Error::other(
            "settings upsert failed",
        ))))?;
    Ok(ApiOk::ok(row))
}

// --------------------------------------------------------------------- stats

fn require_admin(auth: &AuthUser) -> Result<(), ApiError> {
    // Go middlewares.Admin(): 401 "Unauthorized" for non-admins.
    if auth.role == 2 {
        Ok(())
    } else {
        Err(ApiError::Unauthorized("Unauthorized".into()))
    }
}

fn last12_month_keys(now_utc: (i32, u8)) -> Vec<String> {
    // (year, month 1..=12) → the 12 "YYYY-MM" keys ending at the current month.
    let (mut y, mut m) = now_utc;
    let mut keys = Vec::with_capacity(12);
    for _ in 0..12 {
        keys.push(format!("{y:04}-{m:02}"));
        m -= 1;
        if m == 0 {
            m = 12;
            y -= 1;
        }
    }
    keys.reverse();
    keys
}

fn fill_missing_months(series: &[repo::MonthlyCount], keys: &[String]) -> Vec<repo::MonthlyCount> {
    keys.iter()
        .map(|month| repo::MonthlyCount {
            month: month.clone(),
            count: series
                .iter()
                .find(|r| &r.month == month)
                .map(|r| r.count)
                .unwrap_or(0),
        })
        .collect()
}

fn growth_window_keys() -> Vec<String> {
    let now = time::OffsetDateTime::now_utc();
    last12_month_keys((now.year(), now.month() as u8))
}

/// GET /stats/overview
pub async fn overview_stats(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    let total_users = repo::users_total(&state.db).await?;
    let total_nodes = repo::nodes_total(&state.db).await?;
    let total_size = repo::nodes_total_size(&state.db).await?;
    Ok(ApiOk::ok(json!({
        "total_users": total_users,
        "total_nodes": total_nodes,
        "total_size": total_size,
    })))
}

/// GET /stats/users
pub async fn user_stats(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    let total_users = repo::users_total(&state.db).await?;
    let keys = growth_window_keys();
    let since = month_key_to_ms(keys.first())?;
    let growth = repo::users_monthly_growth(&state.db, since).await?;
    Ok(ApiOk::ok(json!({
        "total_users": total_users,
        "growth_last_12_months": fill_missing_months(&growth, &keys),
    })))
}

/// GET /stats/nodes?top=
pub async fn node_stats(
    State(state): State<AppState>,
    auth: AuthUser,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<ApiOk<serde_json::Value>, ApiError> {
    require_admin(&auth)?;
    let mut top = 5i32;
    if let Some(top_str) = q.get("top").filter(|v| !v.is_empty()) {
        top = top_str
            .parse()
            .map_err(|_| ApiError::BadRequest("invalid top limit".into()))?;
    }

    let total_nodes = repo::nodes_total(&state.db).await?;
    let total_size = repo::nodes_total_size(&state.db).await?;
    let keys = growth_window_keys();
    let since = month_key_to_ms(keys.first())?;
    let growth = repo::nodes_monthly_growth(&state.db, since).await?;
    let top_nodes = repo::top_users_by_node_count(&state.db, top).await?;
    let top_size = repo::top_users_by_node_size(&state.db, top).await?;

    Ok(ApiOk::ok(json!({
        "total_nodes": total_nodes,
        "total_size": total_size,
        "growth_last_12_months": fill_missing_months(&growth, &keys),
        "top_users_by_nodes": top_nodes,
        "top_users_by_size": top_size,
    })))
}

/// First day of the given month, UTC, in epoch milliseconds (Go
/// startOfMonthUTC(...).AddDate(0,-11,0) window start).
fn month_key_to_ms(key: Option<&String>) -> Result<i64, ApiError> {
    let Some(key) = key else {
        return Err(ApiError::Internal(Box::new(std::io::Error::other(
            "no months",
        ))));
    };
    let (y, m) = key
        .split_once('-')
        .and_then(|(y, m)| Some((y.parse::<i32>().ok()?, m.parse::<u8>().ok()?)))
        .ok_or(ApiError::Internal(Box::new(std::io::Error::other(
            "bad month key",
        ))))?;
    let date = time::Date::from_calendar_date(
        y,
        time::Month::try_from(m)
            .map_err(|_| ApiError::Internal(Box::new(std::io::Error::other("bad month"))))?,
        1,
    )
    .map_err(|_| ApiError::Internal(Box::new(std::io::Error::other("bad date"))))?;
    let dt = date.with_time(time::Time::MIDNIGHT).assume_utc();
    Ok(dt.unix_timestamp() * 1000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last12_keys_are_ordered_and_rolling() {
        let keys = last12_month_keys((2026, 11));
        assert_eq!(keys.len(), 12);
        assert_eq!(keys.first(), Some(&"2025-12".to_string()));
        assert_eq!(keys.last(), Some(&"2026-11".to_string()));
    }

    #[test]
    fn fill_missing_defaults_to_zero() {
        let series = vec![repo::MonthlyCount {
            month: "2026-10".into(),
            count: 7,
        }];
        let keys = last12_month_keys((2026, 11));
        let filled = fill_missing_months(&series, &keys);
        assert_eq!(filled.len(), 12);
        let oct = filled.iter().find(|r| r.month == "2026-10").unwrap();
        assert_eq!(oct.count, 7);
        assert!(filled.iter().all(|r| r.month != "2026-11" || r.count == 0));
    }

    #[test]
    fn year_boundary_rolls_back() {
        let keys = last12_month_keys((2026, 1));
        assert_eq!(keys.first(), Some(&"2025-02".to_string()));
        assert_eq!(keys.last(), Some(&"2026-01".to_string()));
        assert_eq!(keys.len(), 12);
    }

    #[test]
    fn month_key_to_ms_known_value() {
        // 2026-11-01T00:00:00Z = 1793491200.
        let ms = month_key_to_ms(Some(&"2026-11".to_string())).unwrap();
        assert_eq!(ms, 1793491200 * 1000);
        assert!(month_key_to_ms(None).is_err());
        assert!(month_key_to_ms(Some(&"not-a-month".to_string())).is_err());
        assert!(month_key_to_ms(Some(&"2026-13".to_string())).is_err());
    }

    #[test]
    fn require_admin_gates_role() {
        let admin = AuthUser {
            id: uuid::Uuid::nil(),
            role: 2,
        };
        assert!(require_admin(&admin).is_ok());
        let user = AuthUser {
            id: uuid::Uuid::nil(),
            role: 1,
        };
        assert!(matches!(
            require_admin(&user),
            Err(ApiError::Unauthorized(_))
        ));
    }
}
