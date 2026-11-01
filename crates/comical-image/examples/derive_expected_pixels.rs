//! Derives `output_rgb_hex` for the cases in `tests/vectors/images.json`.
//!
//! The vectors were recorded from an oracle outside this repository, and the
//! hashes stay the source of truth. This tool only runs where the implementation
//! already reproduces every recorded hash exactly, so the bytes it writes are
//! provably the oracle's bytes; on any other platform it refuses instead of
//! blessing local output.

use std::path::PathBuf;

use comical_image::{Filter, RgbImage, resize};
use serde_json::Value;
use sha2::{Digest, Sha256};

fn hex_to_bytes(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(text, "{byte:02x}");
    }
    text
}

fn sha256_hex(bytes: &[u8]) -> String {
    bytes_to_hex(&Sha256::digest(bytes))
}

fn filter_from_name(name: &str) -> Filter {
    match name {
        "box" => Filter::Box,
        "bilinear" => Filter::Bilinear,
        "bicubic" => Filter::Bicubic,
        "bspline" => Filter::BSpline,
        "catmull_rom" => Filter::CatmullRom,
        "lanczos3" => Filter::Lanczos3,
        other => panic!("unknown filter: {other}"),
    }
}

fn main() {
    let path = PathBuf::from("tests/vectors/images.json");
    let data = std::fs::read_to_string(&path).unwrap();
    let mut root: Value = serde_json::from_str(&data).unwrap();
    let source = &root["source"];
    let width = source["width"].as_u64().unwrap() as u32;
    let height = source["height"].as_u64().unwrap() as u32;
    let pixels = hex_to_bytes(source["rgb_hex"].as_str().unwrap());
    let image = RgbImage::new(width, height, pixels).unwrap();

    let mut mismatched = Vec::new();
    for case in root["cases"].as_array_mut().unwrap() {
        let name = case["name"].as_str().unwrap().to_owned();
        let filter = filter_from_name(case["filter"].as_str().unwrap());
        let target_width = case["target"]["width"].as_u64().unwrap() as u32;
        let target_height = case["target"]["height"].as_u64().unwrap() as u32;
        let recorded = case["output_sha256"].as_str().unwrap().to_owned();

        let output = resize(&image, target_width, target_height, filter).unwrap();
        let hash = sha256_hex(output.pixels());
        if hash != recorded {
            mismatched.push(name);
            continue;
        }

        case.as_object_mut().unwrap().insert(
            "output_rgb_hex".to_owned(),
            Value::String(bytes_to_hex(output.pixels())),
        );
        eprintln!("{name}: {hash}");
    }

    assert!(
        mismatched.is_empty(),
        "this build does not reproduce the oracle for {mismatched:?}; refusing to record pixels"
    );

    std::fs::write(&path, serde_json::to_string_pretty(&root).unwrap() + "\n").unwrap();
    eprintln!("Updated {}", path.display());
}
