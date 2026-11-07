mod config;
mod db;
mod error;
mod logger;
mod routes;
mod state;
mod types;

use std::net::SocketAddr;

use axum::routing::get;
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

    let state = AppState { db: pool };

    let app = Router::new()
        .route("/api/health", get(routes::health::health))
        .with_state(state)
        .layer(tower_http::trace::TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!(%addr, "listening");
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind failed");
    axum::serve(listener, app)
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
