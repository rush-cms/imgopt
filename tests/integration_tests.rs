use axum::{body::Body, http::Request};
use tower::ServiceExt;
use imgopt::server::create_router;
use std::net::TcpListener;
use tokio::net::TcpListener as TokioTcpListener;
use reqwest::Client;

const TEST_TOKEN: &str = "test_token";

#[tokio::test]
async fn test_health() {
    let app = create_router();
    let response = app.oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn test_convert_webp() {
    let listener = TokioTcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{}/convert", addr);

    tokio::spawn(async move {
        axum::serve(listener, create_router()).await.unwrap();
    });

    // small delay for server start
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Create a dummy 1x1 PNG image
    let png_bytes = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A,
        0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
        0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
        0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41,
        0x54, 0x08, 0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00,
        0x00, 0x03, 0x01, 0x01, 0x00, 0x18, 0xDD, 0x8D,
        0xB0, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
        0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    std::env::set_var("API_TOKEN", TEST_TOKEN);

    let client = Client::new();
    let form = reqwest::multipart::Form::new()
        .part("file", reqwest::multipart::Part::bytes(png_bytes.clone()).file_name("test.png"));

    let resp = client.post(&url)
        .header("Authorization", format!("Bearer {}", TEST_TOKEN))
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    assert_eq!(resp.headers().get("content-type").unwrap(), "image/webp");
    let bytes = resp.bytes().await.unwrap();
    assert!(bytes.len() > 0);
    assert_eq!(&bytes[0..4], b"RIFF");
}

#[tokio::test]
async fn test_convert_avif() {
     let listener = TokioTcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{}/convert", addr);

    tokio::spawn(async move {
        axum::serve(listener, create_router()).await.unwrap();
    });

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let png_bytes = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A,
        0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
        0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
        0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41,
        0x54, 0x08, 0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00,
        0x00, 0x03, 0x01, 0x01, 0x00, 0x18, 0xDD, 0x8D,
        0xB0, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
        0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    std::env::set_var("API_TOKEN", TEST_TOKEN);

    let client = Client::new();
    let form = reqwest::multipart::Form::new()
        .part("file", reqwest::multipart::Part::bytes(png_bytes).file_name("test.png"))
        .text("format", "avif");

    let resp = client.post(&url)
        .header("Authorization", format!("Bearer {}", TEST_TOKEN))
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    assert_eq!(resp.headers().get("content-type").unwrap(), "image/avif");
    let bytes = resp.bytes().await.unwrap();
    assert!(bytes.len() > 0);
    // 4..8 usually "ftyp"
    assert_eq!(&bytes[4..8], b"ftyp");
}
