//! Entity identifiers: UUIDv7 everywhere (decision 2026-11-12).
//!
//! All tables use UUID primary keys, generated application-side. UUIDv7 is
//! time-ordered (index-friendly) and coordination-free (no worker-id
//! assignment). On the wire ids are plain JSON strings —
//! `uuid::Uuid`'s serde impl handles that, so no custom module is needed.

/// New entity id (UUIDv7: unix-ms time prefix + randomness).
pub fn new_id() -> uuid::Uuid {
    uuid::Uuid::now_v7()
}
