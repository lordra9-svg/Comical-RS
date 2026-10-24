//! Decoding PNG, JPEG and GIF bytes to opaque RGB8.

use std::io::Cursor;

use crate::{ImageError, RgbImage};

/// Upper bound on decoded pixels accepted by [`decode`].
///
/// The limit is checked from the image header before the decoder allocates the
/// pixel buffer. At three bytes per pixel it allows up to about 286 MiB of RGB
/// output.
pub const MAX_DECODED_PIXELS: u64 = 100_000_000;

/// Reads the pixel dimensions of PNG, JPEG or GIF bytes without decoding the
/// image data.
///
/// The same format checks and pixel limit as [`decode`] apply, so a header
/// accepted here can be decoded later. This is used to lay out a page before
/// its full-resolution decode finishes.
pub fn probe_size(bytes: &[u8]) -> Result<(u32, u32), ImageError> {
    let (_, width, height) = format_and_dimensions(bytes)?;
    Ok((width, height))
}

fn format_and_dimensions(
    bytes: &[u8],
) -> Result<(image::ImageFormat, u32, u32), ImageError> {
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

    let (width, height) = image::ImageReader::with_format(Cursor::new(bytes), format)
        .into_dimensions()
        .map_err(decode_failure)?;

    if width == 0 || height == 0 {
        return Err(ImageError::InvalidDimensions { width, height });
    }
    let pixel_count = u64::from(width) * u64::from(height);
    if pixel_count > MAX_DECODED_PIXELS {
        return Err(ImageError::TooLarge { width, height });
    }

    Ok((format, width, height))
}

/// Decodes PNG, JPEG or GIF bytes to an opaque RGB image.
///
/// Alpha is ignored, not composited: an RGBA pixel keeps its colour channels
/// and simply loses its alpha byte. For GIF input the first frame is decoded.
/// Unsupported, truncated or malformed data returns an [`ImageError`] instead
/// of panicking.
pub fn decode(bytes: &[u8]) -> Result<RgbImage, ImageError> {
    let (format, header_width, header_height) = format_and_dimensions(bytes)?;

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
