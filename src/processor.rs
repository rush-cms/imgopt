use image::ImageReader;
use imgref::Img;
use rgb::FromSlice;
use std::io::Cursor;
use webp::Encoder;

pub const MAX_DIMENSION: u32 = 4096;
const MAX_PIXELS: u64 = 16_000_000; // ~4K resolution safety cap

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputFormat {
    WebP,
    Avif,
}

#[derive(Debug)]
pub struct ProcessOptions {
    pub quality: f32,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub format: OutputFormat,
}

pub fn process_image(bytes: &[u8], options: ProcessOptions) -> anyhow::Result<Vec<u8>> {
    // SEC-002: validate requested dimensions before any processing
    if let Some(w) = options.width {
        if w == 0 || w > MAX_DIMENSION {
            return Err(anyhow::anyhow!(
                "width {} is out of range (1–{})",
                w,
                MAX_DIMENSION
            ));
        }
    }
    if let Some(h) = options.height {
        if h == 0 || h > MAX_DIMENSION {
            return Err(anyhow::anyhow!(
                "height {} is out of range (1–{})",
                h,
                MAX_DIMENSION
            ));
        }
    }
    if let (Some(w), Some(h)) = (options.width, options.height) {
        if (w as u64) * (h as u64) > MAX_PIXELS {
            return Err(anyhow::anyhow!(
                "Requested {}x{} exceeds maximum pixel count",
                w,
                h
            ));
        }
    }

    // Clamp quality to a valid encoder range
    let quality = options.quality.clamp(1.0, 100.0);

    // 1. Decode image
    let img = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()?
        .decode()?;

    // SEC-002: validate the actual decoded dimensions (guards against decompression bombs)
    let orig_w = img.width();
    let orig_h = img.height();
    if orig_w > MAX_DIMENSION || orig_h > MAX_DIMENSION {
        return Err(anyhow::anyhow!(
            "Source image {}x{} exceeds maximum allowed {}x{}",
            orig_w,
            orig_h,
            MAX_DIMENSION,
            MAX_DIMENSION
        ));
    }
    if (orig_w as u64) * (orig_h as u64) > MAX_PIXELS {
        return Err(anyhow::anyhow!("Source image pixel count exceeds maximum"));
    }

    // 2. Resize if requested
    let img = if let (Some(w), Some(h)) = (options.width, options.height) {
        img.resize_exact(w, h, image::imageops::FilterType::Lanczos3)
    } else if let Some(w) = options.width {
        img.resize(w, u32::MAX, image::imageops::FilterType::Lanczos3)
    } else if let Some(h) = options.height {
        img.resize(u32::MAX, h, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };

    // 3. Encode and record duration for observability
    let encode_start = std::time::Instant::now();

    let result = match options.format {
        OutputFormat::WebP => {
            let encoder = Encoder::from_image(&img)
                .map_err(|e| anyhow::anyhow!("WebP encoding failed: {}", e))?;
            let webp_memory = encoder.encode(quality);
            Ok(webp_memory.to_vec())
        }
        OutputFormat::Avif => {
            let rgba = img.to_rgba8();
            let width = rgba.width() as usize;
            let height = rgba.height() as usize;
            let raw = rgba.as_raw();
            let pixels = raw.as_rgba();

            let img_ref = Img::new(pixels, width, height);

            // Speed 6: faster encoding with acceptable quality for server-side use
            let result = ravif::Encoder::new()
                .with_quality(quality)
                .with_speed(6)
                .encode_rgba(img_ref)
                .map_err(|e| anyhow::anyhow!("AVIF encoding failed: {}", e))?;

            Ok(result.avif_file)
        }
    };

    tracing::debug!(
        format = ?options.format,
        duration_ms = encode_start.elapsed().as_millis(),
        "Encoding completed"
    );

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};

    fn create_test_image() -> Vec<u8> {
        let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(100, 100);
        for pixel in img.pixels_mut() {
            *pixel = Rgba([255, 0, 0, 255]);
        }
        let mut bytes = Vec::new();
        img.write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        bytes
    }

    #[test]
    fn test_process_webp() {
        let input = create_test_image();
        let options = ProcessOptions {
            quality: 80.0,
            width: None,
            height: None,
            format: OutputFormat::WebP,
        };
        let result = process_image(&input, options).unwrap();
        assert!(!result.is_empty());
        assert_eq!(&result[0..4], b"RIFF");
        assert_eq!(&result[8..12], b"WEBP");
    }

    #[test]
    fn test_process_avif() {
        let input = create_test_image();
        let options = ProcessOptions {
            quality: 80.0,
            width: None,
            height: None,
            format: OutputFormat::Avif,
        };
        let result = process_image(&input, options).unwrap();
        assert!(!result.is_empty());
        assert_eq!(&result[4..8], b"ftyp");
        assert_eq!(&result[8..12], b"avif");
    }

    #[test]
    fn test_resize() {
        let input = create_test_image();
        let options = ProcessOptions {
            quality: 80.0,
            width: Some(50),
            height: Some(50),
            format: OutputFormat::WebP,
        };
        let result = process_image(&input, options).unwrap();
        let decoded = ImageReader::new(Cursor::new(result))
            .with_guessed_format()
            .unwrap()
            .decode()
            .unwrap();
        assert_eq!(decoded.width(), 50);
        assert_eq!(decoded.height(), 50);
    }

    #[test]
    fn test_dimension_too_large_rejected() {
        let input = create_test_image();
        let options = ProcessOptions {
            quality: 80.0,
            width: Some(MAX_DIMENSION + 1),
            height: None,
            format: OutputFormat::WebP,
        };
        let result = process_image(&input, options);
        assert!(result.is_err());
    }

    #[test]
    fn test_quality_clamped() {
        let input = create_test_image();
        // quality=150 should be clamped to 100, not return an error
        let options = ProcessOptions {
            quality: 150.0,
            width: None,
            height: None,
            format: OutputFormat::WebP,
        };
        let result = process_image(&input, options);
        assert!(result.is_ok());
    }
}
