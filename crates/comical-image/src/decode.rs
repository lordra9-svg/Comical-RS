//! Decoding PNG, JPEG, GIF and WebP bytes to opaque RGB8.

use std::io::Cursor;

use crate::rotate::Rotation;
use crate::{ImageError, RgbImage};

/// Upper bound on decoded pixels accepted by [`decode`].
///
/// The limit is checked from the image header before the decoder allocates the
/// pixel buffer. At three bytes per pixel it allows up to about 286 MiB of RGB
/// output.
pub const MAX_DECODED_PIXELS: u64 = 100_000_000;

/// Reads the pixel dimensions of PNG, JPEG, GIF or WebP bytes without decoding the
/// image data.
///
/// When `apply_exif` is true and the image carries an EXIF orientation tag,
/// the returned dimensions are swapped for orientations that rotate the image
/// 90 or 270 degrees. The same format checks and pixel limit as [`decode`]
/// apply, so a header accepted here can be decoded later. This is used to lay
/// out a page before its full-resolution decode finishes.
pub fn probe_size(bytes: &[u8], apply_exif: bool) -> Result<(u32, u32), ImageError> {
    let (_, width, height) = format_and_dimensions(bytes)?;
    if apply_exif {
        let rotation = exif_rotation(bytes);
        if rotation.swaps_dimensions() {
            return Ok((height, width));
        }
    }
    Ok((width, height))
}

/// Returns the EXIF orientation as a [`Rotation`], or [`Rotation::None`] when
/// no orientation tag is present or the value is not one of 1–8.
///
/// Only JPEG (and TIFF-embedded) images carry EXIF; other formats return
/// [`Rotation::None`] without error. Parse failures are silently ignored so a
/// corrupt EXIF segment never blocks decoding.
pub fn exif_rotation(bytes: &[u8]) -> Rotation {
    let ms = match nom_exif::MediaSource::from_memory(bytes.to_vec()) {
        Ok(ms) => ms,
        Err(_) => return Rotation::None,
    };
    let mut parser = nom_exif::MediaParser::new();
    let iter = match parser.parse_exif(ms) {
        Ok(iter) => iter,
        Err(_) => return Rotation::None,
    };
    let exif: nom_exif::Exif = iter.into();
    let orientation = exif
        .get(nom_exif::ExifTag::Orientation)
        .and_then(|v| v.as_u16());
    match orientation {
        Some(1) => Rotation::None,
        Some(2) => Rotation::None, // mirrored horizontally; we do not flip, treat as none
        Some(3) => Rotation::Cw180,
        Some(4) => Rotation::Cw180, // mirrored vertically; approximate with 180
        Some(5) => Rotation::Cw90,  // mirrored + 90 CW; approximate with 90
        Some(6) => Rotation::Cw90,
        Some(7) => Rotation::Cw270, // mirrored + 270 CW; approximate with 270
        Some(8) => Rotation::Cw270,
        _ => Rotation::None,
    }
}

fn format_and_dimensions(bytes: &[u8]) -> Result<(image::ImageFormat, u32, u32), ImageError> {
    if bytes.is_empty() {
        return Err(ImageError::EmptyInput);
    }

    let format = image::guess_format(bytes)
        .map_err(|_| ImageError::UnsupportedFormat("unrecognised image data".to_owned()))?;
    if !matches!(
        format,
        image::ImageFormat::Png
            | image::ImageFormat::Jpeg
            | image::ImageFormat::Gif
            | image::ImageFormat::WebP
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

/// Decodes PNG, JPEG, GIF or WebP bytes to an opaque RGB image.
///
/// When `apply_exif` is true and the image carries an EXIF orientation tag,
/// the decoded pixels are rotated to match before returning. Alpha is ignored,
/// not composited: an RGBA pixel keeps its colour channels and simply loses
/// its alpha byte. For GIF input the first frame is decoded. Unsupported,
/// truncated or malformed data returns an [`ImageError`] instead of panicking.
pub fn decode(bytes: &[u8], apply_exif: bool) -> Result<RgbImage, ImageError> {
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
    let image = RgbImage::new(width, height, rgb.into_raw())?;

    if apply_exif {
        let rotation = exif_rotation(bytes);
        if rotation != Rotation::None {
            return Ok(crate::rotate::rotate(&image, rotation));
        }
    }
    Ok(image)
}

fn decode_failure(error: image::ImageError) -> ImageError {
    match error {
        image::ImageError::Unsupported(inner) => ImageError::UnsupportedFormat(inner.to_string()),
        other => ImageError::Decode(other.to_string()),
    }
}
