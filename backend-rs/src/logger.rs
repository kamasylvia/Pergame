//! Structured logging (tracing). `RUST_LOG` env filter, default `info`.

/// Initialize tracing subscriber (`RUST_LOG`, default `info`).
pub fn init() {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();
}
