//! Startup admin-role reconciliation (Go `userService.LoadAppAdmins` parity).
//!
//! `ADMIN_ACCOUNTS` (comma-separated UUID strings) is the source of truth:
//! existing admins not on the list are demoted to the regular role (1), listed
//! users are promoted to admin (2). Bit-flag values mirror
//! `permissions/constants.go`: RoleNone = 1<<0 = 1, RoleAdministrator = 1<<1 = 2.

use sqlx::PgPool;
use toasty::Db;
use uuid::Uuid;

const ROLE_NONE: i32 = 1; // 1 << 0
const ROLE_ADMIN: i32 = 2; // 1 << 1

pub async fn load_app_admins(db: &mut Db, sql: &PgPool) {
    let admin_ids: Vec<String> = std::env::var("ADMIN_ACCOUNTS")
        .map(|v| {
            v.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();

    // Demote existing admins that are not in the current list (sqlx escape
    // hatch: toasty 0.11 lacks a generic list-by-field query surface).
    let rows = sqlx::query_as::<_, (Uuid, String)>("SELECT id, username FROM users WHERE role = $1")
        .bind(ROLE_ADMIN)
        .fetch_all(sql)
        .await;
    let Ok(rows) = rows else {
        tracing::warn!("app-admins: failed to load existing admin users");
        return;
    };
    for (id, username) in rows {
        let id_str = id.to_string();
        if !admin_ids.contains(&id_str) {
            match crate::models::User::filter_by_id(id)
                .update()
                .role(ROLE_NONE)
                .exec(db)
                .await
            {
                Ok(_) => tracing::info!("app-admins: user {} set to regular role", username),
                Err(e) => tracing::error!(error = %e, %username, "failed to demote user"),
            }
        }
    }

    // Promote users listed in ADMIN_ACCOUNTS.
    for id_str in &admin_ids {
        let Ok(id) = Uuid::parse_str(id_str) else {
            tracing::warn!(%id_str, "app-admins: invalid admin user ID");
            continue;
        };
        match crate::models::User::filter_by_id(id)
            .first()
            .exec(db)
            .await
        {
            Ok(Some(user)) => {
                if user.role != ROLE_ADMIN {
                    match crate::models::User::filter_by_id(user.id)
                        .update()
                        .role(ROLE_ADMIN)
                        .exec(db)
                        .await
                    {
                        Ok(_) => {
                            tracing::info!("app-admins: user {} granted admin role", user.username)
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, username = %user.username, "failed to promote user")
                        }
                    }
                }
            }
            _ => {
                tracing::warn!(%id_str, "app-admins: failed to load admin user (check ID and existence)");
            }
        }
    }
}
