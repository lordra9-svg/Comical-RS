//! Decoding PNG, JPEG and GIF bytes to opaque RGB8.

use std::io::Cursor;

use crate::{ImageError, RgbImage};

/// Upper bound on decoded pixels accepted by [`decode`].
///
/// The limit is checked from the image header before the decoder allocates the
/// pixel buffer. At three bytes per pixel it allows up to about 286 MiB of RGB
/// output.
pub const MAX_DECODED_PIXELS: u64 = 100_000_000;

/// Decodes PNG, JPEG or GIF bytes to an opaque RGB image.
///
/// Alpha is ignored, not composited: an RGBA pixel keeps its colour channels
/// and simply loses its alpha byte. For GIF input the first frame is decoded.
/// Unsupported, truncated or malformed data returns an [`ImageError`] instead
/// of panicking.
pub fn decode(bytes: &[u8]) -> Result<RgbImage, ImageError> {
    if bytes.is_empty() {
        return Err(ImageError::EmptyInput);
    }

    let format = image::guess_format(bytes)
        .map_err(|_| ImageError::UnsupportedFormat("unrecognised image data".to_owned()))?;
    if !matches!(
        format,
        image::ImageFormat::Png | image::ImageFormat::Jpeg | image::ImageFormat::Gif
    ) {
        return Err(ImageError::UnsupportedFormat(format!("{format:?}")));
    }

    let (header_width, header_height) = image::ImageReader::with_format(Cursor::new(bytes), format)
        .into_dimensions()
        .map_err(decode_failure)?;

    if header_width == 0 || header_height == 0 {
        return Err(ImageError::InvalidDimensions {
            width: header_width,
            height: header_height,
        });
    }
    let pixel_count = u64::from(header_width) * u64::from(header_height);
    if pixel_count > MAX_DECODED_PIXELS {
        return Err(ImageError::TooLarge {
            width: header_width,
            height: header_height,
        });
    }

    let mut limits = image::Limits::no_limits();
    limits.max_image_width = Some(header_width);
    limits.max_image_height = Some(header_height);
    limits.max_alloc = Some(MAX_DECODED_PIXELS.saturating_mul(4));
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(limits);
    let decoded = reader.decode().map_err(|error| match error {
        image::ImageError::Limits(_) => ImageError::TooLarge {
            width: header_width,
            height: header_height,
        },
        other => decode_failure(other),
    })?;

    let rgb = decoded.to_rgb8();
    let (width, height) = rgb.dimensions();
    RgbImage::new(width, height, rgb.into_raw())
}

fn decode_failure(error: image::ImageError) -> ImageError {
    match error {
        image::ImageError::Unsupported(inner) => ImageError::UnsupportedFormat(inner.to_string()),
        other => ImageError::Decode(other.to_string()),
    }
}
