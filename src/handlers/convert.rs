use axum::{
    body::Bytes,
    extract::Multipart,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use std::time::Duration;
use uuid::Uuid;

use crate::processor::{process_image, OutputFormat, ProcessOptions, MAX_DIMENSION};

// SEC-003: maximum time allowed for a single encoding operation
const ENCODING_TIMEOUT: Duration = Duration::from_secs(30);

pub async fn convert_image(mut multipart: Multipart) -> Response {
    let request_id = Uuid::new_v4();

    let mut file_bytes: Option<Bytes> = None;
    let mut quality = 80.0f32;
    let mut width: Option<u32> = None;
    let mut height: Option<u32> = None;
    let mut strip = true;
    let mut format = OutputFormat::WebP;

    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        let name = field.name().unwrap_or("").to_string();

        match name.as_str() {
            "file" => match field.bytes().await {
                Ok(bytes) => file_bytes = Some(bytes),
                Err(e) => {
                    tracing::warn!(%request_id, error = %e, "Failed to read file field");
                    return (StatusCode::BAD_REQUEST, "Failed to read uploaded file")
                        .into_response();
                }
            },
            "quality" => {
                if let Ok(val) = field.text().await {
                    match val.parse::<f32>() {
                        Ok(q) if (1.0..=100.0).contains(&q) => quality = q,
                        Ok(_) => {
                            return (StatusCode::BAD_REQUEST, "quality must be between 1 and 100")
                                .into_response()
                        }
                        Err(_) => {
                            return (StatusCode::BAD_REQUEST, "quality must be a number")
                                .into_response()
                        }
                    }
                }
            }
            "width" => {
                if let Ok(val) = field.text().await {
                    match val.parse::<u32>() {
                        Ok(w) if w > 0 && w <= MAX_DIMENSION => width = Some(w),
                        Ok(0) => {
                            return (StatusCode::BAD_REQUEST, "width must be greater than 0")
                                .into_response()
                        }
                        Ok(_) => {
                            return (
                                StatusCode::BAD_REQUEST,
                                format!("width must not exceed {}", MAX_DIMENSION),
                            )
                                .into_response()
                        }
                        Err(_) => {
                            return (StatusCode::BAD_REQUEST, "width must be a positive integer")
                                .into_response()
                        }
                    }
                }
            }
            "height" => {
                if let Ok(val) = field.text().await {
                    match val.parse::<u32>() {
                        Ok(h) if h > 0 && h <= MAX_DIMENSION => height = Some(h),
                        Ok(0) => {
                            return (StatusCode::BAD_REQUEST, "height must be greater than 0")
                                .into_response()
                        }
                        Ok(_) => {
                            return (
                                StatusCode::BAD_REQUEST,
                                format!("height must not exceed {}", MAX_DIMENSION),
                            )
                                .into_response()
                        }
                        Err(_) => {
                            return (StatusCode::BAD_REQUEST, "height must be a positive integer")
                                .into_response()
                        }
                    }
                }
            }
            "strip" => {
                if let Ok(val) = field.text().await {
                    strip = val.parse::<bool>().unwrap_or(true);
                }
            }
            "format" => {
                if let Ok(val) = field.text().await {
                    match val.to_lowercase().as_str() {
                        "avif" => format = OutputFormat::Avif,
                        _ => format = OutputFormat::WebP,
                    }
                }
            }
            _ => {}
        }
    }

    let Some(bytes) = file_bytes else {
        tracing::warn!(%request_id, "Request missing required file field");
        return (StatusCode::BAD_REQUEST, "Missing file field").into_response();
    };

    tracing::info!(
        %request_id,
        format = ?format,
        ?width,
        ?height,
        quality,
        file_size = bytes.len(),
        "Processing image"
    );

    let options = ProcessOptions {
        quality,
        width,
        height,
        _strip_metadata: strip,
        format,
    };
    let format_copy = format;

    // SEC-003: wrap spawn_blocking with a timeout to prevent CPU starvation
    let processing = tokio::task::spawn_blocking(move || process_image(&bytes, options));

    match tokio::time::timeout(ENCODING_TIMEOUT, processing).await {
        Ok(Ok(Ok(converted_bytes))) => {
            tracing::info!(
                %request_id,
                output_size = converted_bytes.len(),
                "Image conversion successful"
            );
            let content_type = match format_copy {
                OutputFormat::WebP => "image/webp",
                OutputFormat::Avif => "image/avif",
            };
            let mut headers = HeaderMap::new();
            headers.insert("Content-Type", content_type.parse().unwrap());
            // OBS-001: propagate request_id to client for traceability
            headers.insert("X-Request-Id", request_id.to_string().parse().unwrap());
            (StatusCode::OK, headers, converted_bytes).into_response()
        }
        Ok(Ok(Err(e))) => {
            tracing::error!(%request_id, error = %e, "Image processing failed");
            (StatusCode::UNPROCESSABLE_ENTITY, "Image processing failed").into_response()
        }
        Ok(Err(e)) => {
            tracing::error!(%request_id, error = %e, "Task join error");
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response()
        }
        Err(_) => {
            tracing::error!(
                %request_id,
                timeout_secs = ENCODING_TIMEOUT.as_secs(),
                "Image encoding timed out"
            );
            (StatusCode::REQUEST_TIMEOUT, "Processing timed out").into_response()
        }
    }
}
