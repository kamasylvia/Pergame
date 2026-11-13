//! Per-user upload quotas (users.max_file_size / max_total_size).
//!
//! Read chain, per limit: user column → `[Cdn]` config → built-in default.
//! `None` at the user level means "not set, fall through". At the config
//! level, `0` means unlimited (empty/missing is unlimited project-wide; a
//! literal `0` quota would forbid all uploads, so config `0` disables the
//! check instead). Admins (role=2) bypass both checks entirely.

use sqlx::PgPool;
use uuid::Uuid;

/// Resolved limits for one upload decision. `None` = unlimited.
pub struct Quota {
    pub max_file: Option<u64>,
    pub max_total: Option<u64>,
}

/// Zero at the config level disables the check (unlimited).
fn config_limit(value: u64) -> Option<u64> {
    if value == 0 { None } else { Some(value) }
}

/// Resolve quotas for `user_id` with `role`. Admins get unlimited.
pub async fn for_user(pool: &PgPool, user_id: Uuid, role: i32) -> Quota {
    if crate::auth::guard::is_app_admin(role) {
        return Quota {
            max_file: None,
            max_total: None,
        };
    }
    let cfg = crate::config::global();
    let row: Option<(Option<i64>, Option<i64>)> =
        sqlx::query_as("SELECT max_file_size, max_total_size FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
    let (user_file, user_total) = row.unwrap_or((None, None));
    Quota {
        max_file: user_file
            .and_then(|v| u64::try_from(v).ok())
            .map_or_else(|| config_limit(cfg.cdn.max_size), Some),
        max_total: user_total
            .and_then(|v| u64::try_from(v).ok())
            .map_or_else(|| config_limit(cfg.cdn.max_uploads_size), Some),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_config_is_unlimited() {
        assert_eq!(config_limit(0), None);
        assert_eq!(config_limit(100), Some(100));
    }
}
