//! Turning an untrusted input file into the image Foundry stores: format check, EXIF
//! orientation, downscale, re-encode (which also drops all metadata). Used for Bakery images
//! and agent avatars alike.

use std::io::Cursor;

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits};

use super::{AppError, ErrorCode};

/// Inputs over this are refused before decoding.
pub const MAX_INPUT_BYTES: u64 = 25 * 1024 * 1024;
/// Stored images are at most this many pixels on their long edge.
pub const MAX_EDGE: u32 = 1600;
const JPEG_QUALITY: u8 = 85;
/// Refuse absurd dimensions before allocating (decompression bombs).
const MAX_DECODE_EDGE: u32 = 20_000;

pub struct Processed {
    pub bytes: Vec<u8>,
    /// `png` or `jpg`.
    pub ext: &'static str,
    pub width: u32,
    pub height: u32,
}

pub fn too_large(what: &str) -> AppError {
    AppError::new(
        ErrorCode::FileTooLarge,
        format!("{what} is over the 25 MB limit"),
    )
}

fn unsupported(message: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::UnsupportedImage, message)
}

/// Decodes, orients, downscales and re-encodes. PNG stays PNG, JPEG is re-encoded at
/// quality 85, WebP becomes PNG. The format is judged by the bytes, not the file name.
pub fn process(input: &[u8]) -> Result<Processed, AppError> {
    if input.len() as u64 > MAX_INPUT_BYTES {
        return Err(too_large("this image"));
    }
    let mut reader = ImageReader::new(Cursor::new(input))
        .with_guessed_format()
        .map_err(|err| unsupported(format!("could not read the image: {err}")))?;
    let (format, ext) = match reader.format() {
        Some(ImageFormat::Png) => (ImageFormat::Png, "png"),
        Some(ImageFormat::Jpeg) => (ImageFormat::Jpeg, "jpg"),
        Some(ImageFormat::WebP) => (ImageFormat::Png, "png"),
        _ => return Err(unsupported("only PNG, JPEG and WebP images are accepted")),
    };
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DECODE_EDGE);
    limits.max_image_height = Some(MAX_DECODE_EDGE);
    reader.limits(limits);

    let decode_err =
        |err: image::ImageError| unsupported(format!("could not decode the image: {err}"));
    let mut decoder = reader.into_decoder().map_err(decode_err)?;
    let orientation = decoder.orientation().map_err(decode_err)?;
    let mut img = DynamicImage::from_decoder(decoder).map_err(decode_err)?;
    img.apply_orientation(orientation);

    if img.width().max(img.height()) > MAX_EDGE {
        // resize() keeps the aspect ratio and fits inside the box.
        img = img.resize(MAX_EDGE, MAX_EDGE, FilterType::Lanczos3);
    }

    let mut bytes = Vec::new();
    let encoded = match format {
        ImageFormat::Jpeg => DynamicImage::ImageRgb8(img.to_rgb8())
            .write_with_encoder(JpegEncoder::new_with_quality(&mut bytes, JPEG_QUALITY)),
        _ => img.write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png),
    };
    encoded.map_err(|err| AppError::internal(format!("could not encode the image: {err}")))?;
    Ok(Processed {
        bytes,
        ext,
        width: img.width(),
        height: img.height(),
    })
}
