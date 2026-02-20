use image::io::Reader as ImageReader;
use std::io::Cursor;
use webp::Encoder;
use imgref::Img;
use rgb::FromSlice; // Import FromSlice trait for as_rgba()

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
    pub _strip_metadata: bool,
    pub format: OutputFormat,
}

pub fn process_image(bytes: &[u8], options: ProcessOptions) -> anyhow::Result<Vec<u8>> {
    // 1. Decode image
    let img = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()?
        .decode()?;

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

    // 3. Encode
    match options.format {
        OutputFormat::WebP => {
            let encoder = Encoder::from_image(&img).map_err(|e| anyhow::anyhow!("WebP encoding failed: {}", e))?;
            let webp_memory = encoder.encode(options.quality);
            Ok(webp_memory.to_vec())
        },
        OutputFormat::Avif => {
            let rgba = img.to_rgba8();
            let width = rgba.width() as usize;
            let height = rgba.height() as usize;
            let raw = rgba.as_raw();
            let pixels = raw.as_rgba(); // Safe conversion from &[u8] to &[RGBA8] via rgb crate
            
            let img_ref = Img::new(pixels, width, height);
            
            // Speed 4 is a good balance for server-side encoding (0=slowest/best, 10=fastest/worst)
            // Quality is 1-100
            let result = ravif::Encoder::new()
                .with_quality(options.quality)
                .with_speed(4) 
                .encode_rgba(img_ref)
                .map_err(|e| anyhow::anyhow!("AVIF encoding failed: {}", e))?;
                
            Ok(result.avif_file)
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, ImageBuffer};

    fn create_test_image() -> Vec<u8> {
        let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(100, 100);
        for pixel in img.pixels_mut() {
            *pixel = Rgba([255, 0, 0, 255]);
        }
        let mut bytes = Vec::new();
        img.write_to(&mut Cursor::new(&mut bytes), image::ImageOutputFormat::Png).unwrap();
        bytes
    }

    #[test]
    fn test_process_webp() {
        let input = create_test_image();
        let options = ProcessOptions {
            quality: 80.0,
            width: None,
            height: None,
            _strip_metadata: true,
            format: OutputFormat::WebP,
        };
        let result = process_image(&input, options).unwrap();
        assert!(result.len() > 0);
        // Check WebP magic bytes
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
            _strip_metadata: true,
            format: OutputFormat::Avif,
        };
        let result = process_image(&input, options).unwrap();
        assert!(result.len() > 0);
        // Check AVIF magic bytes (ftypavif is usually at index 4)
        // 0-3: size
        // 4-7: "ftyp"
        // 8-11: "avif"
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
            _strip_metadata: true,
            format: OutputFormat::WebP,
        };
        let result = process_image(&input, options).unwrap();
        let decoded = ImageReader::new(Cursor::new(result)).with_guessed_format().unwrap().decode().unwrap();
        assert_eq!(decoded.width(), 50);
        assert_eq!(decoded.height(), 50);
    }
}
