//! Typed errors for decoding and resampling.

use thiserror::Error;

/// An image decoding or resampling failure.
#[derive(Debug, Error)]
pub enum ImageError {
    /// The input was empty.
    #[error("image data is empty")]
    EmptyInput,

    /// The bytes are not one of the supported formats.
    #[error("unsupported image format: {0}")]
    UnsupportedFormat(String),

    /// The decoder rejected the data.
    #[error("could not decode image: {0}")]
    Decode(String),

    /// A dimension was zero or otherwise unusable.
    #[error("invalid image dimensions: {width}x{height}")]
    InvalidDimensions { width: u32, height: u32 },

    /// A resize target had a zero side.
    #[error("invalid target size: {width}x{height}; both sides must be at least 1")]
    InvalidTarget { width: u32, height: u32 },

    /// The pixel buffer length did not match the dimensions.
    #[error("image buffer is {actual} bytes, expected {expected}")]
    BufferLength { expected: usize, actual: usize },

    /// The image or target is too large to allocate.
    #[error("image is too large: {width}x{height}")]
    TooLarge { width: u32, height: u32 },
}
