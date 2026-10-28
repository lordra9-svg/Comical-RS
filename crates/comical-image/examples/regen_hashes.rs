use comical_image::{Filter, RgbImage, resize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

fn hex_to_bytes(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}
fn sha256_hex(bytes: &[u8]) -> String {
    let d = Sha256::digest(bytes);
    d.iter().map(|b| format!("{b:02x}")).collect()
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
    let w = source["width"].as_u64().unwrap() as u32;
    let h = source["height"].as_u64().unwrap() as u32;
    let pixels = hex_to_bytes(source["rgb_hex"].as_str().unwrap());
    let image = RgbImage::new(w, h, pixels).unwrap();
    let cases = root["cases"].as_array_mut().unwrap();
    for case in cases.iter_mut() {
        let name = case["name"].as_str().unwrap().to_owned();
        let filter = filter_from_name(case["filter"].as_str().unwrap());
        let tw = case["target"]["width"].as_u64().unwrap() as u32;
        let th = case["target"]["height"].as_u64().unwrap() as u32;
        let output = resize(&image, tw, th, filter).unwrap();
        let hash = sha256_hex(output.pixels());
        case.as_object_mut()
            .unwrap()
            .insert("output_sha256".to_owned(), Value::String(hash.clone()));
        eprintln!("{name}: {hash}");
    }
    std::fs::write(&path, serde_json::to_string_pretty(&root).unwrap() + "\n").unwrap();
    eprintln!("Updated {}", path.display());
}
