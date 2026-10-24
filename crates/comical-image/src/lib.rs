//! Image decoding, rotation and resampling.
//!
//! The crate decodes PNG, JPEG and GIF data to an opaque 8-bit RGB buffer,
//! rotates by quarter turns and resizes with a separable two-pass convolution.
//! Alpha is ignored: it is dropped, never composited.

mod decode;
mod error;
mod resize;
mod rotate;

use std::fmt;

pub use decode::{MAX_DECODED_PIXELS, decode, probe_size};
pub use error::ImageError;
pub use resize::{Filter, resize};
pub use rotate::{Rotation, rotate};

/// An opaque 8-bit RGB image in row-major order.
///
/// Pixels are stored as interleaved red, green and blue bytes with no alpha
/// channel. The buffer length is always `width * height * 3`.
#[derive(Clone, PartialEq, Eq)]
pub struct RgbImage {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl RgbImage {
    /// Creates an image from a row-major RGB8 buffer.
    ///
    /// Returns [`ImageError::InvalidDimensions`] for a zero side and
    /// [`ImageError::BufferLength`] when the buffer length does not match the
    /// dimensions.
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, ImageError> {
        if width == 0 || height == 0 {
            return Err(ImageError::InvalidDimensions { width, height });
        }
        let expected = rgb_len(width, height).ok_or(ImageError::TooLarge { width, height })?;
        if pixels.len() != expected {
            return Err(ImageError::BufferLength {
                expected,
                actual: pixels.len(),
            });
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    /// Image width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Image height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The row-major RGB8 pixel buffer.
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// Consumes the image and returns the row-major RGB8 buffer.
    pub fn into_pixels(self) -> Vec<u8> {
        self.pixels
    }

    /// Returns the RGB value at `(x, y)`, or `None` when out of bounds.
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 3]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = rgb_index(x, y, self.width);
        let slice = self.pixels.get(index..index + 3)?;
        Some([slice[0], slice[1], slice[2]])
    }
}

impl fmt::Debug for RgbImage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RgbImage")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("pixels", &format_args!("{} bytes", self.pixels.len()))
            .finish()
    }
}

/// Byte length of a row-major RGB8 buffer, or `None` on overflow.
pub(crate) fn rgb_len(width: u32, height: u32) -> Option<usize> {
    let len = u64::from(width)
        .checked_mul(u64::from(height))?
        .checked_mul(3)?;
    usize::try_from(len).ok()
}

/// Byte index of the first channel of `(x, y)` in an RGB8 buffer.
pub(crate) fn rgb_index(x: u32, y: u32, width: u32) -> usize {
    // A valid RgbImage guarantees width * height * 3 fits in usize, so these
    // casts cannot truncate on any supported target.
    (y as usize * width as usize + x as usize) * 3
}

/// Allocates a zeroed RGB8 buffer, reporting allocation failure as an error.
pub(crate) fn allocate_rgb(width: u32, height: u32) -> Result<Vec<u8>, ImageError> {
    let len = rgb_len(width, height).ok_or(ImageError::TooLarge { width, height })?;
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(len)
        .map_err(|_| ImageError::TooLarge { width, height })?;
    pixels.resize(len, 0);
    Ok(pixels)
}
