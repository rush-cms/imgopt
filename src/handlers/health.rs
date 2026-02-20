use axum::{response::Json, http::StatusCode};
use serde::Serialize;
use std::time::SystemTime;

#[derive(Serialize)]
pub struct HealthResponse {
    status: String,
    version: String,
    uptime_seconds: u64,
}

static START_TIME: std::sync::OnceLock<SystemTime> = std::sync::OnceLock::new();

pub async fn health_check() -> (StatusCode, Json<HealthResponse>) {
    let start = START_TIME.get_or_init(SystemTime::now);
    let uptime = SystemTime::now()
        .duration_since(*start)
        .unwrap_or_default()
        .as_secs();

    let response = HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: uptime,
    };

    (StatusCode::OK, Json(response))
}
