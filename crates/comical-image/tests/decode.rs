//! Decode tests for PNG, JPEG and GIF input.

use std::io::Cursor;

use comical_image::{ImageError, decode, probe_size};
use image::{DynamicImage, Frame, ImageFormat, Rgba, RgbaImage};

fn encode(image: &DynamicImage, format: ImageFormat) -> Vec<u8> {
    let mut bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), format)
        .expect("encode test fixture");
    bytes
}

fn assert_near(actual: [u8; 3], expected: [u8; 3], tolerance: i32) {
    for channel in 0..3 {
        let difference = i32::from(actual[channel]) - i32::from(expected[channel]);
        assert!(
            difference.abs() <= tolerance,
            "channel {channel}: got {actual:?}, expected {expected:?} within {tolerance}"
        );
    }
}

#[test]
fn png_rgb_decodes_to_exact_pixels() {
    let mut source = image::RgbImage::new(3, 2);
    let colors = [
        [255, 0, 0],
        [0, 255, 0],
        [0, 0, 255],
        [10, 20, 30],
        [40, 50, 60],
        [70, 80, 90],
    ];
    for (index, color) in colors.iter().enumerate() {
        let x = index as u32 % 3;
        let y = index as u32 / 3;
        source.put_pixel(x, y, image::Rgb(*color));
    }

    let bytes = encode(&DynamicImage::ImageRgb8(source), ImageFormat::Png);
    let decoded = decode(&bytes).expect("decode PNG");

    assert_eq!((decoded.width(), decoded.height()), (3, 2));
    for (index, color) in colors.iter().enumerate() {
        let x = index as u32 % 3;
        let y = index as u32 / 3;
        assert_eq!(decoded.pixel(x, y), Some(*color));
    }
}

#[test]
fn png_rgba_ignores_alpha() {
    let mut source = RgbaImage::new(2, 2);
    source.put_pixel(0, 0, Rgba([200, 30, 40, 0]));
    source.put_pixel(1, 0, Rgba([10, 220, 30, 1]));
    source.put_pixel(0, 1, Rgba([20, 40, 230, 128]));
    source.put_pixel(1, 1, Rgba([250, 240, 10, 255]));

    let bytes = encode(&DynamicImage::ImageRgba8(source), ImageFormat::Png);
    let decoded = decode(&bytes).expect("decode RGBA PNG");

    assert_eq!((decoded.width(), decoded.height()), (2, 2));
    assert_eq!(decoded.pixel(0, 0), Some([200, 30, 40]));
    assert_eq!(decoded.pixel(1, 0), Some([10, 220, 30]));
    assert_eq!(decoded.pixel(0, 1), Some([20, 40, 230]));
    assert_eq!(decoded.pixel(1, 1), Some([250, 240, 10]));
}

#[test]
fn jpeg_decodes_with_dimensions_and_approximate_color() {
    let source = image::RgbImage::from_pixel(32, 24, image::Rgb([120, 80, 200]));
    let bytes = encode(&DynamicImage::ImageRgb8(source), ImageFormat::Jpeg);
    let decoded = decode(&bytes).expect("decode JPEG");

    assert_eq!((decoded.width(), decoded.height()), (32, 24));
    let center = decoded.pixel(16, 12).expect("center pixel");
    assert_near(center, [120, 80, 200], 12);
}

#[test]
fn gif_decodes_the_first_frame() {
    let first = RgbaImage::from_pixel(4, 4, Rgba([200, 30, 40, 255]));
    let second = RgbaImage::from_pixel(4, 4, Rgba([10, 200, 60, 255]));

    let mut bytes = Vec::new();
    {
        let mut encoder = image::codecs::gif::GifEncoder::new(&mut bytes);
        encoder
            .encode_frame(Frame::new(first))
            .expect("encode first GIF frame");
        encoder
            .encode_frame(Frame::new(second))
            .expect("encode second GIF frame");
    }

    let decoded = decode(&bytes).expect("decode GIF");
    assert_eq!((decoded.width(), decoded.height()), (4, 4));
    let pixel = decoded.pixel(2, 2).expect("center pixel");
    assert_near(pixel, [200, 30, 40], 2);
    assert!(
        pixel[0] > 150,
        "decoded the second frame instead of the first"
    );
}

#[test]
fn probe_size_matches_decoded_dimensions_without_decoding() {
    let source = image::RgbImage::from_pixel(37, 19, image::Rgb([9, 8, 7]));
    let bytes = encode(&DynamicImage::ImageRgb8(source), ImageFormat::Png);
    assert_eq!(probe_size(&bytes).expect("probe PNG"), (37, 19));
}

#[test]
fn probe_size_rejects_empty_and_unsupported_input() {
    assert!(matches!(probe_size(&[]), Err(ImageError::EmptyInput)));
    assert!(matches!(
        probe_size(b"not an image"),
        Err(ImageError::UnsupportedFormat(_))
    ));
}

#[test]
fn empty_input_is_an_error() {
    assert!(matches!(decode(&[]), Err(ImageError::EmptyInput)));
}

#[test]
fn garbage_input_is_an_error() {
    let result = decode(b"this is not an image");
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(ImageError::UnsupportedFormat(_)) | Err(ImageError::Decode(_))
    ));
}

#[test]
fn truncated_png_is_an_error() {
    let source = image::RgbImage::from_pixel(8, 8, image::Rgb([1, 2, 3]));
    let bytes = encode(&DynamicImage::ImageRgb8(source), ImageFormat::Png);
    let truncated = &bytes[..bytes.len() / 2];
    assert!(decode(truncated).is_err());
}
