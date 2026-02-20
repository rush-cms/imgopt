use axum::{body::Body, http::Request};
use imgopt::server::create_router;
use reqwest::Client;
use std::time::Duration;
use tokio::net::TcpListener as TokioTcpListener;
use tower::ServiceExt;

const TEST_TOKEN: &str = "integration_test_token";

/// Minimal 1×1 PNG used across tests
const PNG_1X1: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
    0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08, 0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00,
    0x00, 0x03, 0x01, 0x01, 0x00, 0x18, 0xDD, 0x8D, 0xB0, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
    0x44, 0xAE, 0x42, 0x60, 0x82,
];

/// Spawn a test server on a random port and return its base URL.
/// API_TOKEN must be set in the calling test before this is invoked.
async fn spawn_server() -> String {
    let listener = TokioTcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, create_router()).await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    format!("http://{}", addr)
}

// ── health / ready ────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_health() {
    let app = create_router();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn test_ready() {
    let app = create_router();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
}

// ── happy-path conversions ────────────────────────────────────────────────────

#[tokio::test]
async fn test_convert_webp() {
    unsafe { std::env::set_var("API_TOKEN", TEST_TOKEN) };
    let base = spawn_server().await;

    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(PNG_1X1.to_vec()).file_name("test.png"),
    );

    let resp = Client::new()
        .post(format!("{}/convert", base))
        .header("Authorization", format!("Bearer {}", TEST_TOKEN))
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    assert_eq!(resp.headers().get("content-type").unwrap(), "image/webp");
    let bytes = resp.bytes().await.unwrap();
    assert!(!bytes.is_empty());
    assert_eq!(&bytes[0..4], b"RIFF");
}

#[tokio::test]
async fn test_convert_avif() {
    unsafe { std::env::set_var("API_TOKEN", TEST_TOKEN) };
    let base = spawn_server().await;

    let form = reqwest::multipart::Form::new()
        .part(
            "file",
            reqwest::multipart::Part::bytes(PNG_1X1.to_vec()).file_name("test.png"),
        )
        .text("format", "avif");

    let resp = Client::new()
        .post(format!("{}/convert", base))
        .header("Authorization", format!("Bearer {}", TEST_TOKEN))
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    assert_eq!(resp.headers().get("content-type").unwrap(), "image/avif");
    let bytes = resp.bytes().await.unwrap();
    assert!(!bytes.is_empty());
    assert_eq!(&bytes[4..8], b"ftyp");
}

#[tokio::test]
async fn test_response_contains_request_id() {
    unsafe { std::env::set_var("API_TOKEN", TEST_TOKEN) };
    let base = spawn_server().await;

    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(PNG_1X1.to_vec()).file_name("test.png"),
    );

    let resp = Client::new()
        .post(format!("{}/convert", base))
        .header("Authorization", format!("Bearer {}", TEST_TOKEN))
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    assert!(resp.headers().contains_key("x-request-id"));
}

// ── authentication ────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_unauthorized_no_header() {
    unsafe { std::env::set_var("API_TOKEN", TEST_TOKEN) };
    let base = spawn_server().await;

    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(PNG_1X1.to_vec()).file_name("test.png"),
    );

    let resp = Client::new()
        .post(format!("{}/convert", base))
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_unauthorized_wrong_token() {
    unsafe { std::env::set_var("API_TOKEN", TEST_TOKEN) };
    let base = spawn_server().await;

    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(PNG_1X1.to_vec()).file_name("test.png"),
    );

    let resp = Client::new()
        .post(format!("{}/convert", base))
        .header("Authorization", "Bearer wrong_token")
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 401);
}

// ── input validation ──────────────────────────────────────────────────────────

#[tokio::test]
async fn test_missing_file_field() {
    unsafe { std::env::set_var("API_TOKEN", TEST_TOKEN) };
    let base = spawn_server().await;

    let form = reqwest::multipart::Form::new().text("quality", "80");

    let resp = Client::new()
        .post(format!("{}/convert", base))
        .header("Authorization", format!("Bearer {}", TEST_TOKEN))
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_quality_out_of_range() {
    unsafe { std::env::set_var("API_TOKEN", TEST_TOKEN) };
    let base = spawn_server().await;

    let form = reqwest::multipart::Form::new()
        .part(
            "file",
            reqwest::multipart::Part::bytes(PNG_1X1.to_vec()).file_name("test.png"),
        )
        .text("quality", "150");

    let resp = Client::new()
        .post(format!("{}/convert", base))
        .header("Authorization", format!("Bearer {}", TEST_TOKEN))
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_width_too_large() {
    unsafe { std::env::set_var("API_TOKEN", TEST_TOKEN) };
    let base = spawn_server().await;

    let form = reqwest::multipart::Form::new()
        .part(
            "file",
            reqwest::multipart::Part::bytes(PNG_1X1.to_vec()).file_name("test.png"),
        )
        .text("width", "99999");

    let resp = Client::new()
        .post(format!("{}/convert", base))
        .header("Authorization", format!("Bearer {}", TEST_TOKEN))
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_height_zero_rejected() {
    unsafe { std::env::set_var("API_TOKEN", TEST_TOKEN) };
    let base = spawn_server().await;

    let form = reqwest::multipart::Form::new()
        .part(
            "file",
            reqwest::multipart::Part::bytes(PNG_1X1.to_vec()).file_name("test.png"),
        )
        .text("height", "0");

    let resp = Client::new()
        .post(format!("{}/convert", base))
        .header("Authorization", format!("Bearer {}", TEST_TOKEN))
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}
