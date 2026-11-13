//! Restore member sections shared by apply: permissions + invitations.
//!
//! Extracted from `apply.rs` so the main engine file stays under the 600-line
//! cap. Both sections only touch rows whose node id survived the node pass.

use serde_json::Value;
use uuid::Uuid;

use crate::error::ApiError;

use super::{RestoreReport, as_i64, as_string, as_uuid};

type Tx<'a> = sqlx::Transaction<'a, sqlx::Postgres>;

/// Restore permission grants (grantees resolved by username).
pub(crate) async fn restore_permissions(
    tx: &mut Tx<'_>,
    permissions_raw: &[Value],
    node_ids: &std::collections::HashSet<uuid::Uuid>,
    report: &mut RestoreReport,
) -> Result<(), ApiError> {
    // ---- permissions: node must be in the archive; grantee resolved by
    // username (unknown users skipped, never fabricated). ON CONFLICT skips
    // duplicates inside the archive itself.
    {
        let mut user_cache: std::collections::HashMap<String, Option<Uuid>> =
            std::collections::HashMap::new();
        for p in permissions_raw {
            let (Some(node_id), Some(grantee), level) = (
                as_uuid(p.get("node_id")),
                as_string(p.get("grantee")),
                as_i64(p.get("permission")).unwrap_or(1) as i32,
            ) else {
                report.permissions_skipped.push("missing fields".into());
                continue;
            };
            if !node_ids.contains(&node_id) {
                report
                    .permissions_skipped
                    .push(format!("{grantee}:node not in archive"));
                continue;
            }
            let grantee_id = match user_cache.get(&grantee) {
                Some(cached) => *cached,
                None => {
                    let found: Option<(Uuid,)> =
                        sqlx::query_as("SELECT id FROM users WHERE username = $1")
                            .bind(&grantee)
                            .fetch_optional(&mut **tx)
                            .await
                            .unwrap_or(None);
                    let id = found.map(|r| r.0);
                    user_cache.insert(grantee.clone(), id);
                    id
                }
            };
            let Some(grantee_id) = grantee_id else {
                report
                    .permissions_skipped
                    .push(format!("{grantee}:unknown user"));
                continue;
            };
            let inserted = sqlx::query(
                r#"INSERT INTO permissions (id, node_id, user_id, permission, created_timestamp)
                   VALUES ($1,$2,$3,$4,$5) ON CONFLICT (node_id, user_id) DO NOTHING"#,
            )
            .bind(crate::types::new_id())
            .bind(node_id)
            .bind(grantee_id)
            .bind(level)
            .bind(as_i64(p.get("created_timestamp")).unwrap_or(0))
            .execute(&mut **tx)
            .await
            .map(|r| r.rows_affected())
            .unwrap_or(0);
            if inserted == 0 {
                report
                    .permissions_skipped
                    .push(format!("{grantee}:duplicate"));
            } else {
                report.permissions += 1;
            }
        }
    }
    Ok(())
}

/// Restore invitation codes (deduped by code).
pub(crate) async fn restore_invitations(
    tx: &mut Tx<'_>,
    invitations_raw: &[Value],
    node_ids: &std::collections::HashSet<uuid::Uuid>,
    report: &mut RestoreReport,
) -> Result<(), ApiError> {
    // ---- invitations: self-contained rows, deduped by code. A conflicting
    // code means the same invitation already exists — skip, the old code
    // stays valid and nothing is lost.
    for inv in invitations_raw {
        let (Some(code), Some(node_id)) = (
            as_string(inv.get("invitation_code")),
            as_uuid(inv.get("node_id")),
        ) else {
            report.invitations_skipped.push("missing fields".into());
            continue;
        };
        if !node_ids.contains(&node_id) {
            report
                .invitations_skipped
                .push(format!("{code}:node not in archive"));
            continue;
        }
        let inserted = sqlx::query(
            r#"INSERT INTO node_invitations (id, invitation_code, permission_level, node_id, created_timestamp)
               VALUES ($1,$2,$3,$4,$5) ON CONFLICT (invitation_code) DO NOTHING"#,
        )
        .bind(crate::types::new_id())
        .bind(&code)
        .bind(as_i64(inv.get("permission_level")).unwrap_or(1) as i32)
        .bind(node_id)
        .bind(as_i64(inv.get("created_timestamp")).unwrap_or(0))
        .execute(&mut **tx)
        .await
        .map(|r| r.rows_affected())
        .unwrap_or(0);
        if inserted == 0 {
            report.invitations_skipped.push(format!("{code}:duplicate"));
        } else {
            report.invitations += 1;
        }
    }
    Ok(())
}
