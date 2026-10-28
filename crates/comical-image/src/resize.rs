//! Separable two-pass resampling.

use std::f64::consts::PI;
use std::sync::LazyLock;

use crate::{ImageError, RgbImage, allocate_rgb};

static SRGB_TO_LINEAR: LazyLock<[f32; 256]> = LazyLock::new(|| {
    let mut lut = [0.0f32; 256];
    for (i, slot) in lut.iter_mut().enumerate() {
        let c = i as f32 / 255.0;
        *slot = if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        };
    }
    lut
});

/// Number of intervals in the coarse linear-to-sRGB table.
///
/// `powf` dominated resize time on full comic pages. The coarse table answers
/// almost every channel with one lookup; the few intervals that contain a
/// rounding boundary fall back to an exact threshold search, so the output
/// bytes are unchanged.
const LINEAR_TO_SRGB_INTERVALS: usize = 1 << 16;

/// Smallest linear value whose round-to-nearest sRGB byte is at least the
/// index. Entry 0 is zero and entry 256 is a sentinel above the valid range.
static LINEAR_TO_SRGB_THRESHOLDS: LazyLock<[f32; 257]> = LazyLock::new(|| {
    let mut thresholds = [0.0f32; 257];
    for value in 1..=255u16 {
        let target = value as u8;
        let mut low = 0u32;
        let mut high = 1.0f32.to_bits();
        while low < high {
            let mid = low + (high - low) / 2;
            let candidate = f32::from_bits(mid);
            if exact_linear_to_srgb(candidate) >= target {
                high = mid;
            } else {
                low = mid + 1;
            }
        }
        thresholds[value as usize] = f32::from_bits(low);
    }
    thresholds[256] = f32::INFINITY;
    thresholds
});

static LINEAR_TO_SRGB_COARSE: LazyLock<Box<[u8]>> = LazyLock::new(|| {
    (0..=LINEAR_TO_SRGB_INTERVALS)
        .map(|index| exact_linear_to_srgb(index as f32 / LINEAR_TO_SRGB_INTERVALS as f32))
        .collect::<Vec<_>>()
        .into_boxed_slice()
});

#[inline]
fn exact_linear_to_srgb(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let srgb = if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (srgb * 255.0 + 0.5) as u8
}

#[inline]
fn linear_to_srgb(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let index = (c * LINEAR_TO_SRGB_INTERVALS as f32) as usize;
    if index >= LINEAR_TO_SRGB_INTERVALS {
        return LINEAR_TO_SRGB_COARSE[LINEAR_TO_SRGB_INTERVALS];
    }
    let low = LINEAR_TO_SRGB_COARSE[index];
    if low == LINEAR_TO_SRGB_COARSE[index + 1] {
        return low;
    }

    // This interval contains at least one rounding boundary. Find the exact
    // byte with the precomputed thresholds.
    let thresholds = &*LINEAR_TO_SRGB_THRESHOLDS;
    let mut left = 0usize;
    let mut right = 256usize;
    while left + 1 < right {
        let mid = (left + right) / 2;
        if thresholds[mid] <= c {
            left = mid;
        } else {
            right = mid;
        }
    }
    left as u8
}

/// A resampling filter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Filter {
    /// Box filter.
    Box,
    /// Bilinear filter.
    Bilinear,
    /// Mitchell-Netravali cubic with B = 1/3 and C = 1/3.
    Bicubic,
    /// Cubic B-spline.
    BSpline,
    /// Catmull-Rom cubic with B = 0 and C = 1/2.
    #[default]
    CatmullRom,
    /// Lanczos filter with a radius of 3.
    Lanczos3,
}

impl Filter {
    fn support(self) -> f64 {
        match self {
            Self::Box => 0.5,
            Self::Bilinear => 1.0,
            Self::Bicubic | Self::BSpline | Self::CatmullRom => 2.0,
            Self::Lanczos3 => 3.0,
        }
    }

    fn kernel(self, x: f64) -> f64 {
        match self {
            Self::Box => {
                if x.abs() <= 0.5 {
                    1.0
                } else {
                    0.0
                }
            }
            Self::Bilinear => {
                let x = x.abs();
                if x < 1.0 { 1.0 - x } else { 0.0 }
            }
            Self::Bicubic => cubic(1.0 / 3.0, 1.0 / 3.0, x),
            Self::BSpline => bspline(x),
            Self::CatmullRom => cubic(0.0, 0.5, x),
            Self::Lanczos3 => {
                let x = x.abs();
                if x < 3.0 {
                    sinc(x) * sinc(x / 3.0)
                } else {
                    0.0
                }
            }
        }
    }
}

/// Generic cubic kernel with tension `b` and sharpness `c`.
///
/// Polynomials are evaluated in nested form in the same order for every call.
fn cubic(b: f64, c: f64, x: f64) -> f64 {
    let x = x.abs();
    if x < 1.0 {
        let p0 = (6.0 - 2.0 * b) / 6.0;
        let p2 = (-18.0 + 12.0 * b + 6.0 * c) / 6.0;
        let p3 = (12.0 - 9.0 * b - 6.0 * c) / 6.0;
        p0 + x * x * (p2 + x * p3)
    } else if x < 2.0 {
        let p0 = (8.0 * b + 24.0 * c) / 6.0;
        let p1 = (-12.0 * b - 48.0 * c) / 6.0;
        let p2 = (6.0 * b + 30.0 * c) / 6.0;
        let p3 = (-b - 6.0 * c) / 6.0;
        p0 + x * (p1 + x * (p2 + x * p3))
    } else {
        0.0
    }
}

/// Cubic B-spline basis.
///
/// This basis has its own closed form rather than going through [`cubic`]. Its
/// nested evaluation order is deliberate: it reproduces the oracle's last-bit
/// behaviour for the exact image vectors.
fn bspline(x: f64) -> f64 {
    let x = x.abs();
    if x < 1.0 {
        (4.0 - x * x * (6.0 - 3.0 * x)) / 6.0
    } else if x < 2.0 {
        let y = 2.0 - x;
        y * y * y / 6.0
    } else {
        0.0
    }
}

fn sinc(x: f64) -> f64 {
    if x == 0.0 {
        1.0
    } else {
        let value = PI * x;
        value.sin() / value
    }
}

/// One destination sample's source window and normalized weights.
struct Contribution {
    start: usize,
    weights: Vec<f32>,
}

/// Builds the source window and weights for one axis.
fn contributions(src_len: usize, dst_len: usize, filter: Filter) -> Vec<Contribution> {
    let scale = dst_len as f64 / src_len as f64;
    let (fscale, support) = if scale < 1.0 {
        (scale, filter.support() / scale)
    } else {
        (1.0, filter.support())
    };
    let last_source = src_len - 1;

    (0..dst_len)
        .map(|dst_index| {
            let center = dst_index as f64 / scale;
            let left = (center - support).floor();
            let right = (center + support).ceil();
            let start = left.max(0.0) as usize;
            let end = (right.min(last_source as f64)) as usize;
            let mut weights = Vec::with_capacity(end.saturating_sub(start) + 1);
            for i in start..=end {
                weights.push(fscale * filter.kernel(fscale * (center - i as f64)));
            }
            let total: f64 = weights.iter().sum();
            if total != 0.0 {
                for weight in &mut weights {
                    *weight /= total;
                }
            }
            let weights = weights.into_iter().map(|weight| weight as f32).collect();
            Contribution { start, weights }
        })
        .collect()
}

/// Horizontal pass: source `src_w x height` becomes `dst_w x height`.
fn horizontal(
    src: &[u8],
    src_w: u32,
    height: u32,
    dst_w: u32,
    filter: Filter,
) -> Result<Vec<u8>, ImageError> {
    let contributions = contributions(src_w as usize, dst_w as usize, filter);
    let mut out = allocate_rgb(dst_w, height)?;
    let src_stride = src_w as usize * 3;
    let dst_stride = dst_w as usize * 3;

    for y in 0..height as usize {
        let src_row = y * src_stride;
        let dst_row = y * dst_stride;
        for (dst_x, contribution) in contributions.iter().enumerate() {
            let mut red = 0.0f32;
            let mut green = 0.0f32;
            let mut blue = 0.0f32;
            for (offset, &weight) in contribution.weights.iter().enumerate() {
                let index = src_row + (contribution.start + offset) * 3;
                red += SRGB_TO_LINEAR[src[index] as usize] * weight;
                green += SRGB_TO_LINEAR[src[index + 1] as usize] * weight;
                blue += SRGB_TO_LINEAR[src[index + 2] as usize] * weight;
            }
            let index = dst_row + dst_x * 3;
            out[index] = linear_to_srgb(red);
            out[index + 1] = linear_to_srgb(green);
            out[index + 2] = linear_to_srgb(blue);
        }
    }

    Ok(out)
}

/// Vertical pass: source `width x src_h` becomes `width x dst_h`.
fn vertical(
    src: &[u8],
    width: u32,
    src_h: u32,
    dst_h: u32,
    filter: Filter,
) -> Result<Vec<u8>, ImageError> {
    let contributions = contributions(src_h as usize, dst_h as usize, filter);
    let mut out = allocate_rgb(width, dst_h)?;
    let stride = width as usize * 3;

    for (dst_y, contribution) in contributions.iter().enumerate() {
        let dst_row = dst_y * stride;
        for x in 0..width as usize {
            let column = x * 3;
            let mut red = 0.0f32;
            let mut green = 0.0f32;
            let mut blue = 0.0f32;
            for (offset, &weight) in contribution.weights.iter().enumerate() {
                let index = (contribution.start + offset) * stride + column;
                red += SRGB_TO_LINEAR[src[index] as usize] * weight;
                green += SRGB_TO_LINEAR[src[index + 1] as usize] * weight;
                blue += SRGB_TO_LINEAR[src[index + 2] as usize] * weight;
            }
            let index = dst_row + column;
            out[index] = linear_to_srgb(red);
            out[index + 1] = linear_to_srgb(green);
            out[index + 2] = linear_to_srgb(blue);
        }
    }

    Ok(out)
}

/// Resizes an image with a separable two-pass convolution.
///
/// The horizontal pass runs first when `dst_w * src_h <= dst_h * src_w`;
/// otherwise the vertical pass runs first. Each pass writes an 8-bit buffer,
/// so the intermediate result is rounded before the second pass.
///
/// Returns [`ImageError::InvalidTarget`] when either target side is zero and
/// [`ImageError::TooLarge`] when the target cannot be allocated.
pub fn resize(
    image: &RgbImage,
    width: u32,
    height: u32,
    filter: Filter,
) -> Result<RgbImage, ImageError> {
    if width == 0 || height == 0 {
        return Err(ImageError::InvalidTarget { width, height });
    }

    let src_w = image.width();
    let src_h = image.height();
    let horizontal_first =
        u64::from(width) * u64::from(src_h) <= u64::from(height) * u64::from(src_w);

    let pixels = if horizontal_first {
        let intermediate = horizontal(image.pixels(), src_w, src_h, width, filter)?;
        vertical(&intermediate, width, src_h, height, filter)?
    } else {
        let intermediate = vertical(image.pixels(), src_w, src_h, height, filter)?;
        horizontal(&intermediate, src_w, height, width, filter)?
    };

    Ok(RgbImage {
        width,
        height,
        pixels,
    })
}
