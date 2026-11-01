//! Rotation and resize validation tests.

use comical_image::{Filter, ImageError, RgbImage, Rotation, resize, rotate, thumbnail};

fn source() -> RgbImage {
    // 2x3 image: each pixel encodes its index so rotations are easy to read.
    let mut pixels = Vec::new();
    for value in 1u8..=6 {
        let base = value * 3;
        pixels.extend_from_slice(&[base, base + 1, base + 2]);
    }
    RgbImage::new(2, 3, pixels).expect("valid source image")
}

fn assert_pixels(image: &RgbImage, expected: &[[u8; 3]]) {
    assert_eq!(
        image.pixels().len(),
        expected.len() * 3,
        "unexpected pixel count"
    );
    for (index, color) in expected.iter().enumerate() {
        let x = index as u32 % image.width();
        let y = index as u32 / image.width();
        assert_eq!(image.pixel(x, y), Some(*color), "pixel ({x}, {y}) mismatch");
    }
}

const P00: [u8; 3] = [3, 4, 5];
const P10: [u8; 3] = [6, 7, 8];
const P01: [u8; 3] = [9, 10, 11];
const P11: [u8; 3] = [12, 13, 14];
const P02: [u8; 3] = [15, 16, 17];
const P12: [u8; 3] = [18, 19, 20];

#[test]
fn rotation_none_keeps_pixels_and_dimensions() {
    let image = rotate(&source(), Rotation::None);
    assert_eq!((image.width(), image.height()), (2, 3));
    assert_pixels(&image, &[P00, P10, P01, P11, P02, P12]);
}

#[test]
fn rotation_cw90_remaps_pixels() {
    let image = rotate(&source(), Rotation::Cw90);
    assert_eq!((image.width(), image.height()), (3, 2));
    assert_pixels(&image, &[P02, P01, P00, P12, P11, P10]);
}

#[test]
fn rotation_cw180_remaps_pixels() {
    let image = rotate(&source(), Rotation::Cw180);
    assert_eq!((image.width(), image.height()), (2, 3));
    assert_pixels(&image, &[P12, P02, P11, P01, P10, P00]);
}

#[test]
fn rotation_cw270_remaps_pixels() {
    let image = rotate(&source(), Rotation::Cw270);
    assert_eq!((image.width(), image.height()), (3, 2));
    assert_pixels(&image, &[P10, P11, P12, P00, P01, P02]);
}

#[test]
fn four_quarter_turns_return_the_source() {
    let original = source();
    let mut image = rotate(&original, Rotation::Cw90);
    for _ in 0..3 {
        image = rotate(&image, Rotation::Cw90);
    }
    assert_eq!((image.width(), image.height()), (2, 3));
    assert_eq!(image.pixels(), original.pixels());
}

/// A smooth gradient large enough for the decimation path to engage.
fn gradient(width: u32, height: u32) -> RgbImage {
    let mut pixels = Vec::with_capacity((width * height * 3) as usize);
    for y in 0..height {
        for x in 0..width {
            pixels.push((x * 255 / width) as u8);
            pixels.push((y * 255 / height) as u8);
            pixels.push(((x + y) * 255 / (width + height)) as u8);
        }
    }
    RgbImage::new(width, height, pixels).expect("valid gradient")
}

#[test]
fn thumbnail_hits_the_requested_size_and_keeps_a_flat_color() {
    let flat = RgbImage::new(1600, 1200, vec![120; 1600 * 1200 * 3]).expect("valid flat image");
    let thumb = thumbnail(&flat, 80, 60, Filter::Box).expect("thumbnail");
    assert_eq!((thumb.width(), thumb.height()), (80, 60));
    assert!(
        thumb.pixels().iter().all(|&byte| byte == 120),
        "a flat color must survive the decimation unchanged"
    );
}

#[test]
fn thumbnail_stays_close_to_a_direct_resize() {
    let image = gradient(400, 300);
    let direct = resize(&image, 40, 30, Filter::CatmullRom).expect("direct resize");
    let fast = thumbnail(&image, 40, 30, Filter::CatmullRom).expect("thumbnail");
    assert_eq!((fast.width(), fast.height()), (40, 30));
    let worst = direct
        .pixels()
        .iter()
        .zip(fast.pixels())
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap_or(0);
    assert!(
        worst <= 4,
        "decimating first moved a channel by {worst} (limit 4)"
    );
}

#[test]
fn thumbnail_without_a_large_downscale_equals_resize() {
    let image = gradient(100, 100);
    let direct = resize(&image, 90, 90, Filter::Lanczos3).expect("direct resize");
    let fast = thumbnail(&image, 90, 90, Filter::Lanczos3).expect("thumbnail");
    assert_eq!(
        fast.pixels(),
        direct.pixels(),
        "a small downscale must take the plain path"
    );
}

#[test]
fn thumbnail_handles_a_partial_edge_block() {
    let image = gradient(1002, 702);
    let thumb = thumbnail(&image, 100, 70, Filter::Bilinear).expect("thumbnail");
    assert_eq!((thumb.width(), thumb.height()), (100, 70));
}

#[test]
fn zero_thumbnail_target_is_rejected() {
    let result = thumbnail(&source(), 0, 5, Filter::Box);
    assert!(matches!(
        result,
        Err(ImageError::InvalidTarget {
            width: 0,
            height: 5
        })
    ));
}

#[test]
fn zero_target_width_is_rejected() {
    let result = resize(&source(), 0, 5, Filter::Box);
    assert!(matches!(
        result,
        Err(ImageError::InvalidTarget {
            width: 0,
            height: 5
        })
    ));
}

#[test]
fn zero_target_height_is_rejected() {
    let result = resize(&source(), 5, 0, Filter::CatmullRom);
    assert!(matches!(
        result,
        Err(ImageError::InvalidTarget {
            width: 5,
            height: 0
        })
    ));
}

#[test]
fn buffer_length_is_validated() {
    let result = RgbImage::new(2, 2, vec![0; 11]);
    assert!(matches!(
        result,
        Err(ImageError::BufferLength {
            expected: 12,
            actual: 11
        })
    ));
}

#[test]
fn zero_image_dimensions_are_rejected() {
    assert!(matches!(
        RgbImage::new(0, 1, Vec::new()),
        Err(ImageError::InvalidDimensions {
            width: 0,
            height: 1
        })
    ));
}
