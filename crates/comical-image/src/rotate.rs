//! Quarter-turn rotation by index remapping.

use crate::RgbImage;

/// A clockwise rotation applied to a page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Rotation {
    /// No rotation.
    #[default]
    None,
    /// 90 degrees clockwise.
    Cw90,
    /// 180 degrees.
    Cw180,
    /// 270 degrees clockwise (90 degrees counter-clockwise).
    Cw270,
}

impl Rotation {
    /// Returns the clockwise angle in degrees.
    pub fn degrees(self) -> u16 {
        match self {
            Self::None => 0,
            Self::Cw90 => 90,
            Self::Cw180 => 180,
            Self::Cw270 => 270,
        }
    }

    /// Returns the rotation that undoes this one.
    pub fn inverse(self) -> Self {
        match self {
            Self::None => Self::None,
            Self::Cw90 => Self::Cw270,
            Self::Cw180 => Self::Cw180,
            Self::Cw270 => Self::Cw90,
        }
    }

    /// Whether this rotation swaps width and height.
    pub const fn swaps_dimensions(self) -> bool {
        matches!(self, Self::Cw90 | Self::Cw270)
    }
}

/// Rotates an image clockwise without interpolation.
///
/// For [`Rotation::Cw90`] the output is `height` wide and `width` high; the
/// same swap applies to [`Rotation::Cw270`]. Every source pixel is copied to
/// exactly one destination pixel.
pub fn rotate(image: &RgbImage, rotation: Rotation) -> RgbImage {
    let (src_w, src_h) = (image.width(), image.height());
    let (dst_w, dst_h) = match rotation {
        Rotation::None | Rotation::Cw180 => (src_w, src_h),
        Rotation::Cw90 | Rotation::Cw270 => (src_h, src_w),
    };
    let src = image.pixels();
    let mut pixels = vec![0u8; src.len()];
    let src_w_usize = src_w as usize;
    let dst_w_usize = dst_w as usize;

    for y in 0..src_h {
        for x in 0..src_w {
            let (dst_x, dst_y) = match rotation {
                Rotation::None => (x, y),
                Rotation::Cw90 => (src_h - 1 - y, x),
                Rotation::Cw180 => (src_w - 1 - x, src_h - 1 - y),
                Rotation::Cw270 => (y, src_w - 1 - x),
            };
            let src_index = (y as usize * src_w_usize + x as usize) * 3;
            let dst_index = (dst_y as usize * dst_w_usize + dst_x as usize) * 3;
            pixels[dst_index..dst_index + 3].copy_from_slice(&src[src_index..src_index + 3]);
        }
    }

    RgbImage {
        width: dst_w,
        height: dst_h,
        pixels,
    }
}
