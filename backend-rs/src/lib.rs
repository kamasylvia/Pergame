//! Pergame backend library (Rust rewrite): axum routes, Toasty + sqlx data
//! layer, snapshot/restore engine, and MCP server. The binary (`main.rs`)
//! only wires these modules into a server; all logic lives here so unit
//! tests and doc-tests exercise the same code as production.
#![deny(unsafe_code)]
// Rust 2024 made `std::env::set_var/remove_var` unsafe; the only unsafe blocks
// in this crate are test-only env mutations under `#[allow(unsafe_code)]` test
// mods (auth::cookies x2, auth::jwt, config, storage, target::owner).
// Production code has zero unsafe. `deny` (not `forbid`) so those scoped
// test allows can apply.

pub mod auth;
pub mod backup;
pub mod blobs;
pub mod config;
pub mod db;
pub mod db_toasty;
pub mod dql;
pub mod error;
pub mod extract;
pub mod locks;
pub mod logger;
pub mod mail;
pub mod mcp;
pub mod models;
pub mod oidc;
pub mod quota;
pub mod repo;
pub mod response;
pub mod restore;
pub mod routes;
pub mod snapcrypto;
pub mod state;
pub mod storage;
pub mod target;
pub mod types;
pub mod utils;

pub use state::AppState;
