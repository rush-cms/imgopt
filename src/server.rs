use axum::{
    routing::{get, post},
    Router,
};
use std::env;
use tokio::net::TcpListener;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

use crate::handlers;
use crate::middleware;

pub fn create_router() -> Router {
    let max_upload_mb: u64 = env::var("MAX_UPLOAD_MB")
        .unwrap_or_else(|_| "10".to_string())
        .parse()
        .unwrap_or(10);

    let max_bytes = max_upload_mb * 1024 * 1024;

    // Read API_TOKEN once here at router-construction time (startup), not per request.
    // main() already validated that the token is set and non-empty before reaching this point.
    let api_token = env::var("API_TOKEN").unwrap_or_default();

    Router::new()
        .route("/health", get(handlers::health::health_check))
        .route("/ready", get(handlers::health::ready_check))
        .route("/convert", post(handlers::convert::convert_image))
        // Layer execution order (outermost first): TraceLayer → BodyLimit → Auth → Handler
        .layer(middleware::auth::AuthLayer::new(api_token))
        .layer(RequestBodyLimitLayer::new(max_bytes as usize))
        .layer(TraceLayer::new_for_http())
}

pub async fn start(addr: &str) -> anyhow::Result<()> {
    let app = create_router();
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to bind to {}: {}", addr, e))?;

    let shutdown_signal = make_shutdown_signal();

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal)
        .await
        .map_err(|e| anyhow::anyhow!("Server error: {}", e))
}

async fn make_shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install CTRL+C handler");
    };

    #[cfg(unix)]
    {
        let terminate = async {
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("Failed to install SIGTERM handler")
                .recv()
                .await;
        };
        tokio::select! {
            _ = ctrl_c => tracing::info!("Received SIGINT, shutting down gracefully"),
            _ = terminate => tracing::info!("Received SIGTERM, shutting down gracefully"),
        }
    }

    #[cfg(not(unix))]
    {
        ctrl_c.await;
        tracing::info!("Received CTRL+C, shutting down gracefully");
    }
}
