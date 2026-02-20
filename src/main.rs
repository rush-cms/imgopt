use dotenvy::dotenv;
use std::env;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use imgopt::server;

#[cfg(target_os = "linux")]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[tokio::main]
async fn main() {
    dotenv().ok();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    // Fail fast: API_TOKEN must be set and non-empty before accepting any traffic
    match env::var("API_TOKEN") {
        Err(_) => {
            tracing::error!("API_TOKEN environment variable is required but not set");
            std::process::exit(1);
        }
        Ok(t) if t.is_empty() => {
            tracing::error!("API_TOKEN must not be empty");
            std::process::exit(1);
        }
        Ok(_) => {}
    }

    let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("0.0.0.0:{}", port);

    tracing::info!(
        addr = %addr,
        version = env!("CARGO_PKG_VERSION"),
        "Starting imgopt server"
    );

    if let Err(e) = server::start(&addr).await {
        tracing::error!(error = %e, "Server terminated with error");
        std::process::exit(1);
    }

    tracing::info!("Server shut down cleanly");
}
