mod auth;
mod backup;
mod config;
mod db;
mod db_toasty;
mod error;
mod extract;
mod locks;
mod logger;
mod mail;
mod models;
mod oidc;
mod repo;
mod response;
mod restore;
mod routes;
mod snapcrypto;
mod state;
mod storage;
mod target;
mod types;
mod utils;

use std::net::SocketAddr;

use axum::Router;

use axum::routing::{delete, get, patch, post, put};

pub use state::AppState;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    logger::init();

    let cfg = config::global();
    let port = cfg.backend_port();

    let pool = match db::connect(cfg).await {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(error = %e, "failed to connect to database");
            std::process::exit(1);
        }
    };

    if let Err(e) = db::migrate(&pool).await {
        tracing::error!(error = %e, "migrations failed");
        std::process::exit(1);
    }
    tracing::info!("migrations applied");

    // Toasty data layer over the same database (schema stays sqlx-owned).
    let toasty_db = match db_toasty::connect(cfg).await {
        Ok(d) => d,
        Err(e) => {
            tracing::error!(error = %e, "failed to initialize toasty");
            std::process::exit(1);
        }
    };

    // S3 storage client (Go exits when unconfigured — parity).
    let storage = match storage::parse_env() {
        Ok(s) => {
            tracing::info!("S3 storage configured");
            Some(s)
        }
        Err(e) => {
            tracing::error!(reason = %e, "S3 storage not configured; aborting");
            std::process::exit(1);
        }
    };

    // OIDC provider discovery (GO OIDC_N_* env parity).
    let oidc_manager = oidc::OidcManager::from_env().await;
    let oidc_manager = if oidc_manager.enabled() {
        Some(std::sync::Arc::new(oidc_manager))
    } else {
        None
    };

    let state = AppState {
        db: pool,
        toasty: toasty_db,
        storage,
        oidc: oidc_manager,
        backups: std::sync::Arc::new(backup::BackupStore::default()),
        locks: locks::WorkspaceLocks::new(),
    };

    // Middleware needs the lock registry; install it once at startup.
    locks::install(state.locks.clone());

    // Admin-role reconciliation (ADMIN_ACCOUNTS env, Go LoadAppAdmins parity).
    auth::admins::load_app_admins(&mut state.toasty.clone(), &state.db).await;

    // S3 buckets bootstrap (public-read policy + backup lifecycle, Go parity).
    if let Some(storage) = &state.storage
        && let Err(e) = storage.ensure_buckets().await
    {
        tracing::error!(error = %e, "S3 buckets bootstrap failed");
        std::process::exit(1);
    }

    // OIDC provider discovery (GO OIDC_N_* env parity).
    let oidc_manager = oidc::OidcManager::from_env().await;
    let _oidc_manager = if oidc_manager.enabled() {
        Some(std::sync::Arc::new(oidc_manager))
    } else {
        None
    };

    // Scheduled user-level snapshots (SNAPSHOT_INTERVAL_HOURS; disabled when
    // unset). Snapshots are full backups direct-written to user targets.
    if let Some(storage) = state.storage.clone() {
        backup::spawn_snapshot_scheduler(state.backups.clone(), state.db.clone(), storage.clone());
        target::fanout::spawn_target_scheduler(state.db.clone(), state.backups.clone(), storage);
    }

    // Trash retention sweep (TRASH_RETENTION_DAYS, 0/unset = keep forever).
    spawn_trash_sweeper(state.clone());

    let app = Router::new()
        .route("/api/health", get(routes::health::health))
        .route(
            "/api/admin/dr-backup/status",
            get(routes::admin_dr::lock_status),
        )
        .route("/api/auth", post(routes::auth::login))
        // POST is the contract (upstream Go registers `POST /refresh` and the
        // frontend refreshes with POST). GET remains as a harmless alias: the
        // Rust port registered GET only, so every token refresh answered 405
        // and the client's error path redirected to /login — which surfaced
        // as "the signup page bounces back to login after a couple of seconds".
        .route(
            "/api/auth/refresh",
            get(routes::auth::refresh).post(routes::auth::refresh),
        )
        .route("/api/auth/logout", post(routes::auth::logout))
        .route(
            "/api/auth/2fa/request",
            post(routes::totp::request_activation),
        )
        .route(
            "/api/auth/2fa/confirm",
            post(routes::totp::confirm_activation),
        )
        .route("/api/auth/2fa/verify", post(routes::totp::verify_totp))
        .route("/api/auth/2fa/disable", post(routes::totp::disable_totp))
        .route(
            "/api/auth/sessions",
            get(routes::sessions_reset::get_sessions),
        )
        .route(
            "/api/auth/sessions/{sessionId}",
            delete(routes::sessions_reset::delete_session),
        )
        .route(
            "/api/auth/logout/all",
            post(routes::sessions_reset::logout_all_devices),
        )
        .route(
            "/api/auth/request-reset",
            post(routes::sessions_reset::request_reset),
        )
        .route(
            "/api/auth/reset-password",
            post(routes::sessions_reset::reset_password),
        )
        .route("/api/auth/oidc/providers", get(routes::oidc::get_providers))
        .route("/api/auth/oidc/linked", get(routes::oidc::get_linked))
        .route(
            "/api/auth/oidc/{provider}/authorize",
            get(routes::oidc::authorize),
        )
        .route(
            "/api/auth/oidc/{provider}/link",
            post(routes::oidc::link_provider),
        )
        .route(
            "/api/auth/oidc/{provider}/callback",
            post(routes::oidc::callback),
        )
        .route(
            "/api/auth/oidc/{provider}/unlink",
            delete(routes::oidc::unlink_provider),
        )
        .route("/api/user/settings", get(routes::settings::get_settings))
        .route("/api/user/settings", put(routes::settings::save_settings))
        .route("/api/stats/overview", get(routes::settings::overview_stats))
        .route("/api/stats/users", get(routes::settings::user_stats))
        .route("/api/stats/nodes", get(routes::settings::node_stats))
        .route("/api/backup", post(routes::backup::create_backup))
        .route(
            "/api/backup/{jobId}",
            get(routes::backup::get_backup_status),
        )
        .route("/api/backup/{jobId}", delete(routes::backup::cancel_backup))
        .route(
            "/api/backup/{jobId}/download",
            get(routes::backup::download_job_zip),
        )
        .route("/api/backup/snapshots", get(routes::backup::list_snapshots))
        .route(
            "/api/backup/snapshots/{id}/download",
            get(routes::backup::download_snapshot),
        )
        .route(
            "/api/backup/snapshots/{id}",
            delete(routes::backup::delete_snapshot),
        )
        .route(
            "/api/backup/snapshots/{id}/restore",
            post(routes::backup::restore_snapshot),
        )
        .route(
            "/api/backup/snapshots/{id}/preview",
            post(routes::backup::preview_snapshot),
        )
        .route(
            "/api/backup/restore/upload",
            post(routes::backup::restore_upload)
                .layer(axum::extract::DefaultBodyLimit::max(256 * 1024 * 1024)),
        )
        .route(
            "/api/backup/restore/upload/preview",
            post(routes::backup::preview_upload)
                .layer(axum::extract::DefaultBodyLimit::max(256 * 1024 * 1024)),
        )
        .route(
            "/api/admin/dr-backup/targets",
            get(routes::admin_dr::list_targets).post(routes::admin_dr::create_target),
        )
        .route(
            "/api/admin/dr-backup/targets/{id}",
            put(routes::admin_dr::update_target).delete(routes::admin_dr::delete_target),
        )
        .route(
            "/api/admin/dr-backup/targets/{id}/delete-check",
            post(routes::admin_dr::delete_check_target),
        )
        .route(
            "/api/admin/dr-backup/targets/{id}/test",
            post(routes::admin_dr::test_target),
        )
        .route(
            "/api/admin/dr-backup/targets/probe-draft",
            post(routes::admin_dr::probe_draft_target),
        )
        .route(
            "/api/admin/dr-backup/targets/{id}/take-ownership",
            post(routes::admin_dr::take_ownership),
        )
        .route(
            "/api/admin/dr-backup/targets/{id}/empty",
            post(routes::admin_dr::empty_target),
        )
        .route(
            "/api/admin/dr-backup/targets/{id}/sync",
            post(routes::admin_dr::sync_target),
        )
        .route(
            "/api/admin/dr-backup/backup-users",
            get(routes::admin_dr::list_backup_users),
        )
        .route(
            "/api/admin/dr-backup/targets/{id}/export/{snapshotId}",
            get(routes::admin_dr::export_target_snapshot),
        )
        // Per-user targets: same store, ownership-scoped (users manage only
        // targets that list them; admins keep the routes above for the rest).
        .route(
            "/api/backup/targets",
            get(routes::user_targets::list_targets).post(routes::user_targets::create_target),
        )
        .route(
            "/api/backup/targets/{id}",
            put(routes::user_targets::update_target).delete(routes::user_targets::delete_target),
        )
        .route(
            "/api/backup/targets/{id}/delete-check",
            post(routes::user_targets::delete_check_target),
        )
        .route(
            "/api/backup/targets/{id}/test",
            post(routes::user_targets::test_target),
        )
        .route(
            "/api/backup/targets/probe-draft",
            post(routes::user_targets::probe_draft_target),
        )
        .route(
            "/api/backup/targets/{id}/empty",
            post(routes::user_targets::empty_target),
        )
        .route(
            "/api/backup/targets/{id}/sync",
            post(routes::user_targets::sync_target),
        )
        .route(
            "/api/backup/targets/{id}/export/{snapshotId}",
            get(routes::user_targets::export_target_snapshot),
        )
        .route("/api/nodes/links", get(routes::links::list_user_links))
        .route("/api/nodes/{nodeId}/links", get(routes::links::get_links))
        .route(
            "/api/nodes/{nodeId}/links",
            post(routes::links::create_link),
        )
        .route(
            "/api/nodes/{nodeId}/links/{linkId}",
            delete(routes::links::delete_link),
        )
        .route("/api/nodes/search", get(routes::nodes::search))
        .route(
            "/api/nodes/user/{userId}",
            get(routes::nodes::get_user_nodes),
        )
        .route(
            "/api/nodes/shared/{userId}",
            get(routes::nodes::get_shared_nodes),
        )
        .route(
            "/api/nodes/public/{nodeId}",
            get(routes::nodes::get_public_node),
        )
        .route(
            "/api/nodes/{nodeId}/permissions",
            get(routes::permissions::get_node_permissions),
        )
        .route(
            "/api/nodes/{nodeId}/permissions",
            post(routes::permissions::create_permission),
        )
        .route(
            "/api/nodes/{nodeId}/permissions/{permId}",
            patch(routes::permissions::update_permission),
        )
        .route(
            "/api/nodes/{nodeId}/permissions/{permId}",
            delete(routes::permissions::delete_permission),
        )
        .route(
            "/api/nodes/{nodeId}/invitations",
            get(routes::permissions::get_node_invitations),
        )
        .route(
            "/api/nodes/{nodeId}/invitations",
            post(routes::permissions::create_invitation),
        )
        .route(
            "/api/nodes/{nodeId}/invitations/{invitationId}",
            delete(routes::permissions::delete_invitation),
        )
        .route(
            "/api/nodes/invitations/join",
            post(routes::permissions::join_invitation),
        )
        .route("/api/nodes", post(routes::nodes::create_node))
        .route("/api/nodes/{nodeId}", get(routes::nodes::get_node))
        .route("/api/nodes/{nodeId}", put(routes::nodes::update_node))
        .route("/api/nodes/{nodeId}", delete(routes::nodes::delete_node))
        .route("/api/trash", get(routes::trash::list_trash))
        .route("/api/trash", delete(routes::trash::empty_trash))
        .route(
            "/api/trash/{nodeId}/restore",
            post(routes::trash::restore_node),
        )
        .route("/api/trash/{nodeId}", delete(routes::trash::purge_node))
        .route("/api/users", post(routes::users::create_user))
        .route("/api/users/@me", get(routes::users::get_me))
        .route("/api/users/{id}", get(routes::users::get_user))
        .route("/api/users/{id}", patch(routes::users_patch::update_user))
        .route("/api/users/{id}", delete(routes::users::delete_user))
        .route(
            "/api/users/{id}/password",
            patch(routes::users_patch::update_password),
        )
        .route("/api/resources", post(routes::resources::upload_file))
        .route(
            "/api/resources/avatar",
            post(routes::resources::upload_avatar),
        )
        .route("/api/resources/{id}", put(routes::resources::update_file))
        .with_state(state)
        .layer(axum::middleware::from_fn(locks::read_only_middleware))
        .layer(cors_layer())
        .layer(tower_http::trace::TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    /// CORS parity with the Go `frontendCORSConfig`: only the configured frontend
    /// origin may read credentialed API responses; public asset hosts are never
    /// trusted. An empty FRONTEND_URL denies all cross-origin access.
    fn cors_layer() -> tower_http::cors::CorsLayer {
        use axum::http::header::{AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE, ORIGIN};
        use axum::http::{HeaderValue, Method};

        let frontend = std::env::var("FRONTEND_URL").unwrap_or_default();
        let origin = frontend.trim().trim_end_matches('/').to_string();

        let mut layer = tower_http::cors::CorsLayer::new()
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::PATCH,
                Method::DELETE,
                Method::OPTIONS,
            ])
            .allow_headers([ORIGIN, CONTENT_LENGTH, CONTENT_TYPE, AUTHORIZATION])
            .expose_headers([CONTENT_LENGTH])
            .allow_credentials(true);
        if let Ok(value) = origin.parse::<HeaderValue>() {
            layer = layer.allow_origin(value);
        }
        layer
    }
    tracing::info!(%addr, "listening");
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind failed");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .expect("server error");
}

/// Trash retention sweeper (spec .work/features/trash.md §2.2). Reads
/// TRASH_RETENTION_DAYS (0/absent = never auto-purge) and
/// TRASH_SWEEP_INTERVAL_HOURS (default 24) from the environment.
fn spawn_trash_sweeper(state: AppState) {
    let days: u64 = std::env::var("TRASH_RETENTION_DAYS")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(30);
    if days == 0 {
        tracing::info!("TRASH_RETENTION_DAYS=0: trash auto-purge disabled");
        return;
    }
    let hours: u64 = std::env::var("TRASH_SWEEP_INTERVAL_HOURS")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .filter(|h| *h > 0)
        .unwrap_or(24);
    tracing::info!(days, hours, "trash retention sweeper enabled");
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(hours * 3600));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            match routes::trash::sweep_expired(&state, days).await {
                Ok(0) => {}
                Ok(n) => tracing::info!(purged = n, "trash sweep purged expired nodes"),
                Err(e) => tracing::error!(error = %e, "trash sweep failed"),
            }
        }
    });
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.ok();
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutdown signal received");
}
