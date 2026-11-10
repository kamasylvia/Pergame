//! Target ownership claim (siyuan issue #15 §3 parity): `refs/owner.json`
//! inside a target's namespace records which instance last claimed it.
//!
//! Why: two deployments can point at the same physical store (backup + DR
//! migration, staging against prod). Only the claiming instance writes
//! snapshots; the others see a foreign owner and refuse, so two cron
//! schedulers never interleave manifests into one lineage. An explicit
//! takeover (`force`) rewrites the marker — "last configured instance wins".
//!
//! Restore, snapshot management and export do NOT require the claim: read
//! paths stay available to anyone who holds the credentials.

use serde::{Deserialize, Serialize};

use super::config::TargetConn;
use crate::error::ApiError;

pub const OWNER_KEY: &str = "refs/owner.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetOwner {
    /// Stable instance identity (INSTANCE_ID env or derived hostname/db pair).
    pub instance_id: String,
    /// Human-readable host label, for operator messages.
    pub host: String,
    /// epoch ms of the claim write.
    pub claimed_at: i64,
}

/// Stable identity for this deployment. `INSTANCE_ID` wins so operators can
/// pin it explicitly in compose; otherwise hostname + database name is a
/// reasonable default (stable across restarts of the same deployment).
pub fn instance_id() -> String {
    if let Ok(v) = std::env::var("INSTANCE_ID")
        && !v.trim().is_empty()
    {
        return v.trim().to_string();
    }
    let host = hostname();
    let db = crate::config::global().database.name.clone();
    format!("{host}/{db}")
}

fn hostname() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| {
            std::fs::read_to_string("/etc/hostname")
                .ok()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        })
        .unwrap_or_else(|| "unknown-host".into())
}

pub fn current_owner() -> TargetOwner {
    TargetOwner {
        instance_id: instance_id(),
        host: hostname(),
        claimed_at: crate::auth::service::now_ms(),
    }
}

/// Reads the claim marker; `Ok(None)` when the scope has never been claimed.
pub async fn read_owner(conn: &TargetConn) -> Result<Option<TargetOwner>, ApiError> {
    match conn.get(OWNER_KEY).await {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes).ok()),
        Err(ApiError::NotFound(_)) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Writes the claim marker for this instance.
pub async fn write_owner(conn: &TargetConn, owner: &TargetOwner) -> Result<(), ApiError> {
    let bytes = serde_json::to_vec_pretty(owner)
        .map_err(|e| ApiError::Internal(Box::new(std::io::Error::other(e.to_string()))))?;
    conn.put(OWNER_KEY, bytes).await
}

/// Claim decision for a write path (fan-out / scheduled backup).
pub enum ClaimOutcome {
    /// We own the scope (or just took it).
    Claimed,
    /// Another live instance owns it; writes are refused.
    Foreign { owner: TargetOwner, took_over: bool },
}

/// Ensures this instance may write to the target scope.
///
/// `force = true` takes over unconditionally (explicit admin action).
/// Without force, an existing foreign claim is honoured — a second instance
/// must be told to take over rather than silently interleaving snapshots.
pub async fn claim(conn: &TargetConn, force: bool) -> Result<ClaimOutcome, ApiError> {
    let mine = current_owner();
    let existing = read_owner(conn).await?;
    match existing {
        None => {
            write_owner(conn, &mine).await?;
            Ok(ClaimOutcome::Claimed)
        }
        Some(o) if o.instance_id == mine.instance_id => {
            write_owner(conn, &mine).await?; // refresh timestamp
            Ok(ClaimOutcome::Claimed)
        }
        Some(o) if force => {
            write_owner(conn, &mine).await?;
            tracing::warn!(
                previous = %o.instance_id,
                current = %mine.instance_id,
                "backup target ownership taken over"
            );
            Ok(ClaimOutcome::Claimed)
        }
        Some(o) => Ok(ClaimOutcome::Foreign {
            owner: o,
            took_over: false,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_id_prefers_env() {
        // SAFETY: single-threaded test; the env var is not read concurrently.
        unsafe { std::env::set_var("INSTANCE_ID", "  pinned-instance  ") };
        assert_eq!(instance_id(), "pinned-instance");
        unsafe { std::env::remove_var("INSTANCE_ID") };
        // Falls back to host/db, never empty.
        let derived = instance_id();
        assert!(!derived.is_empty());
        assert!(derived.contains('/'));
    }
}
