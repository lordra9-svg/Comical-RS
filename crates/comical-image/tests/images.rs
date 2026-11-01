//! Golden image filter vectors from `tests/vectors/images.json`.

use std::path::PathBuf;

use comical_image::{Filter, RgbImage, resize};
use serde_json::Value;
use sha2::{Digest, Sha256};

const EXPECTED_CASES: usize = 42;

/// Largest gap allowed for one channel against the oracle bytes.
///
/// The oracle hashes were recorded on one platform. The sRGB transfer
/// functions go through `powf`, which is the platform libm, so a handful of
/// channels can land on the other side of a rounding boundary elsewhere. One
/// 8-bit step is exactly that failure; two leaves room for a weight that
/// differs by one more ULP without letting a wrong kernel through.
const CHANNEL_TOLERANCE: u8 = 2;

/// Largest share of channels allowed to differ from the oracle at all.
///
/// A last-bit difference touches a few channels; a real filter regression moves
/// whole regions, so this bound fails long before a wrong kernel could pass.
const ALLOWED_DIFFERING_FRACTION: f64 = 0.01;

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

/// Compares one case against the oracle bytes within the tolerances.
///
/// Only reached when the exact hash differs, so a platform that reproduces the
/// oracle bit for bit never depends on this path.
fn assert_within_tolerance(name: &str, actual: &[u8], expected: &[u8]) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "{name}: output length differs from the oracle"
    );

    let mut worst = 0usize;
    let mut worst_diff = 0u16;
    let mut differing = 0usize;
    for (index, (&actual, &expected)) in actual.iter().zip(expected).enumerate() {
        let diff = u16::from(actual.abs_diff(expected));
        if diff > 0 {
            differing += 1;
        }
        if diff > worst_diff {
            worst_diff = diff;
            worst = index;
        }
    }

    let differing_fraction = differing as f64 / actual.len() as f64;
    assert!(
        worst_diff <= u16::from(CHANNEL_TOLERANCE),
        "{name}: channel {} (sample {worst}) differs by {worst_diff}, above the {} step limit",
        worst / 3,
        CHANNEL_TOLERANCE
    );
    assert!(
        differing_fraction <= ALLOWED_DIFFERING_FRACTION,
        "{name}: {differing} of {} channels differ ({:.2}%), above the {:.0}% limit",
        actual.len(),
        differing_fraction * 100.0,
        ALLOWED_DIFFERING_FRACTION * 100.0
    );
}

#[test]
fn every_image_vector_matches_the_oracle() {
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
        let expected_hash = case["output_sha256"].as_str().expect("output hash");
        let expected = hex_to_bytes(
            case["output_rgb_hex"]
                .as_str()
                .expect("output pixels for the case"),
        );

        // The stored pixels are only useful if they are the oracle's bytes.
        assert_eq!(
            sha256_hex(&expected),
            expected_hash,
            "{name}: recorded pixels do not hash to the recorded oracle hash"
        );

        let output = resize(&image, target_width, target_height, filter)
            .unwrap_or_else(|error| panic!("{name}: resize failed: {error}"));

        assert_eq!(output.width(), target_width, "{name}: wrong width");
        assert_eq!(output.height(), target_height, "{name}: wrong height");
        assert_eq!(
            output.pixels().len(),
            expected.len(),
            "{name}: output buffer length"
        );
        assert_eq!(
            expected.len(),
            target_width as usize * target_height as usize * 3,
            "{name}: oracle pixels do not match the target size"
        );

        // Exact on the platform that recorded the vectors, within tolerance
        // where the platform libm shifts a rounding boundary.
        if sha256_hex(output.pixels()) != expected_hash {
            assert_within_tolerance(name, output.pixels(), &expected);
        }
    }
}

#[test]
fn tolerance_absorbs_last_bit_noise() {
    let oracle: Vec<u8> = (0..300).map(|index| (index * 7) as u8).collect();
    let mut actual = oracle.clone();
    actual[0] = oracle[0].wrapping_add(1);
    actual[3] = oracle[3].wrapping_add(2);
    assert_within_tolerance("near-miss", &actual, &oracle);
}

#[test]
#[should_panic(expected = "above the 2 step limit")]
fn tolerance_rejects_a_channel_regression() {
    let oracle: Vec<u8> = (0..300).map(|index| (index * 7) as u8).collect();
    let mut actual = oracle.clone();
    actual[9] = oracle[9].wrapping_add(3);
    assert_within_tolerance("three-step", &actual, &oracle);
}

#[test]
#[should_panic(expected = "channels differ")]
fn tolerance_rejects_a_widespread_shift() {
    let oracle: Vec<u8> = (0..600).map(|index| (index % 100) as u8).collect();
    let actual: Vec<u8> = oracle.iter().map(|value| value + 2).collect();
    assert_within_tolerance("washout", &actual, &oracle);
}
