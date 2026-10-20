//! Golden image filter vectors from `tests/vectors/images.json`.

use std::path::PathBuf;

use comical_image::{Filter, RgbImage, resize};
use serde_json::Value;
use sha2::{Digest, Sha256};

const EXPECTED_CASES: usize = 42;

fn vectors_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/vectors/images.json")
}

fn filter_from_name(name: &str) -> Filter {
    match name {
        "box" => Filter::Box,
        "bilinear" => Filter::Bilinear,
        "bicubic" => Filter::Bicubic,
        "bspline" => Filter::BSpline,
        "catmull_rom" => Filter::CatmullRom,
        "lanczos3" => Filter::Lanczos3,
        other => panic!("unknown filter in vector file: {other}"),
    }
}

fn hex_to_bytes(hex: &str) -> Vec<u8> {
    assert!(hex.len().is_multiple_of(2), "hex string has an odd length");
    (0..hex.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&hex[index..index + 2], 16)
                .unwrap_or_else(|error| panic!("invalid hex at {index}: {error}"))
        })
        .collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut text = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(text, "{byte:02x}");
    }
    text
}

#[test]
fn every_image_vector_matches_exactly() {
    let data = std::fs::read_to_string(vectors_path()).expect("read tests/vectors/images.json");
    let root: Value = serde_json::from_str(&data).expect("parse images.json");

    assert_eq!(
        root["status"], "oracle-confirmed",
        "image vectors are not oracle-confirmed"
    );

    let source = &root["source"];
    let width = source["width"].as_u64().expect("source width") as u32;
    let height = source["height"].as_u64().expect("source height") as u32;
    let rgb_hex = source["rgb_hex"].as_str().expect("source rgb_hex");
    let pixels = hex_to_bytes(rgb_hex);
    assert_eq!(
        pixels.len(),
        width as usize * height as usize * 3,
        "source buffer does not match its dimensions"
    );
    assert_eq!(
        sha256_hex(&pixels),
        source["rgb_sha256"].as_str().expect("source rgb_sha256"),
        "source pixels do not match their recorded hash"
    );
    let image = RgbImage::new(width, height, pixels).expect("valid source image");

    let cases = root["cases"].as_array().expect("cases array");
    assert_eq!(
        cases.len(),
        EXPECTED_CASES,
        "unexpected number of image vectors"
    );

    for case in cases {
        let name = case["name"].as_str().expect("case name");
        let filter = filter_from_name(case["filter"].as_str().expect("case filter"));
        let target_width = case["target"]["width"].as_u64().expect("target width") as u32;
        let target_height = case["target"]["height"].as_u64().expect("target height") as u32;
        let expected = case["output_sha256"].as_str().expect("output hash");

        let output = resize(&image, target_width, target_height, filter)
            .unwrap_or_else(|error| panic!("{name}: resize failed: {error}"));

        assert_eq!(output.width(), target_width, "{name}: wrong width");
        assert_eq!(output.height(), target_height, "{name}: wrong height");
        assert_eq!(
            sha256_hex(output.pixels()),
            expected,
            "{name}: output hash mismatch"
        );
    }
}
