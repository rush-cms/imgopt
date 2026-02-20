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

    Router::new()
        .route("/health", get(handlers::health::health_check))
        .route("/convert", post(handlers::convert::convert_image))
        .layer(middleware::auth::AuthLayer) // Custom auth middleware
        .layer(RequestBodyLimitLayer::new(max_bytes as usize))
        .layer(TraceLayer::new_for_http())
}

pub async fn start(addr: &str) {
    let app = create_router();
    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
