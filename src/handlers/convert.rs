use axum::{
    extract::Multipart,
    http::{StatusCode, HeaderMap},
    response::{IntoResponse, Response},
    body::Bytes,
};
use crate::processor::{process_image, ProcessOptions, OutputFormat};

pub async fn convert_image(mut multipart: Multipart) -> Response {
    let mut file_bytes: Option<Bytes> = None;
    let mut quality = 80.0;
    let mut width: Option<u32> = None;
    let mut height: Option<u32> = None;
    let mut strip = true;
    let mut format = OutputFormat::WebP;

    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        let name = field.name().unwrap_or("").to_string();
        
        match name.as_str() {
            "file" => {
                if let Ok(bytes) = field.bytes().await {
                    file_bytes = Some(bytes);
                }
            },
            "quality" => {
                if let Ok(val) = field.text().await {
                   if let Ok(parsed) = val.parse::<f32>() {
                        quality = parsed;
                   }
                }
            },
            "width" => {
                if let Ok(val) = field.text().await {
                    width = val.parse::<u32>().ok();
                }
            },
            "height" => {
                if let Ok(val) = field.text().await {
                    height = val.parse::<u32>().ok();
                }
            },
            "strip" => {
                if let Ok(val) = field.text().await {
                   strip = val.parse::<bool>().unwrap_or(true);
                }
            },
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

    if let Some(bytes) = file_bytes {
        let options = ProcessOptions {
            quality,
            width,
            height,
            _strip_metadata: strip,
            format,
        };

        let format_copy = format;

        match tokio::task::spawn_blocking(move || {
            process_image(&bytes, options)
        }).await {
            Ok(Ok(converted_bytes)) => {
                let mut headers = HeaderMap::new();
                let content_type = match format_copy {
                    OutputFormat::WebP => "image/webp",
                    OutputFormat::Avif => "image/avif",
                };
                headers.insert("Content-Type", content_type.parse().unwrap());
                (StatusCode::OK, headers, converted_bytes).into_response()
            },
            Ok(Err(e)) => {
                tracing::error!("Image processing failed: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Processing failed").into_response()
            },
            Err(e) => {
                tracing::error!("Task join error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal Error").into_response()
            }
        }
    } else {
        (StatusCode::BAD_REQUEST, "Missing file field").into_response()
    }
}
