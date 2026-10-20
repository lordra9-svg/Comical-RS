//! Rotation and resize validation tests.

use comical_image::{Filter, ImageError, RgbImage, Rotation, resize, rotate};

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
