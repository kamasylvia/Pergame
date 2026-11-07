mod auth;
mod config;
mod db;
mod db_toasty;
mod error;
mod extract;
mod logger;
mod models;
mod repo;
mod response;
mod utils;
mod routes;
mod state;
mod types;

use std::net::SocketAddr;

use axum::routing::{delete, get, patch, post, put};
use axum::Router;

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

    let snowflake = std::sync::Arc::new(types::SnowflakeGenerator::new(types::SNOWFLAKE_EPOCH_MS));
    let state = AppState { db: pool, toasty: toasty_db, snowflake };

    // Admin-role reconciliation (ADMIN_ACCOUNTS env, Go LoadAppAdmins parity).
    auth::admins::load_app_admins(&mut state.toasty.clone(), &state.db).await;

    let app = Router::new()
        .route("/api/health", get(routes::health::health))
        .route("/api/auth", post(routes::auth::login))
        .route("/api/auth/refresh", get(routes::auth::refresh))
        .route("/api/auth/logout", post(routes::auth::logout))
        .route("/api/auth/2fa/request", post(routes::totp::request_activation))
        .route("/api/auth/2fa/confirm", post(routes::totp::confirm_activation))
        .route("/api/auth/2fa/verify", post(routes::totp::verify_totp))
        .route("/api/auth/2fa/disable", post(routes::totp::disable_totp))
        .route("/api/nodes/search", get(routes::nodes::search))
        .route("/api/nodes/user/{userId}", get(routes::nodes::get_user_nodes))
        .route("/api/nodes/shared/{userId}", get(routes::nodes::get_shared_nodes))
        .route("/api/nodes/public/{nodeId}", get(routes::nodes::get_public_node))
        .route("/api/nodes/{nodeId}/permissions", get(routes::nodes::get_node_permissions))
        .route("/api/nodes", post(routes::nodes::create_node))
        .route("/api/nodes/{nodeId}", get(routes::nodes::get_node))
        .route("/api/nodes/{nodeId}", put(routes::nodes::update_node))
        .route("/api/nodes/{nodeId}", delete(routes::nodes::delete_node))
        .route("/api/users", post(routes::users::create_user))
        .route("/api/users/@me", get(routes::users::get_me))
        .route("/api/users/{id}", get(routes::users::get_user))
        .route("/api/users/{id}", patch(routes::users_patch::update_user))
        .route("/api/users/{id}/password", patch(routes::users_patch::update_password))
        .with_state(state)
        .layer(tower_http::trace::TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!(%addr, "listening");
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind failed");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server error");
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
