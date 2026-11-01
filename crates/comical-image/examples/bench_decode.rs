//! Local throughput harness for the decode path.
//!
//! `cargo run -p comical-image --release --example bench_decode` encodes one
//! deterministic page to PNG, JPEG, GIF and WebP, then prints the median and
//! best wall time for `probe_size`, `decode` and the app's per-page flow
//! (decode plus an 80 px thumbnail, built both by a direct `resize` and by
//! `thumbnail`, so the pre-decimation step can be measured against the path it
//! replaced). It is not part of the test gate: timings move with the machine,
//! and CI asserts the golden hashes instead.

use std::hint::black_box;
use std::io::Cursor;
use std::time::Instant;

use comical_image::{Filter, RgbImage, decode, probe_size, resize, thumbnail};
use image::{DynamicImage, Frame, ImageFormat};

/// Deterministic source page: gradients plus a little noise, so the encoders
/// cannot compress it far below a scanned comic page.
fn source(width: u32, height: u32) -> image::RgbImage {
    let mut state = 0x2545_f491_4f6c_dd1du64;
    let mut pixels = image::RgbImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let noise = (state >> 56) as u8;
            let red = ((x * 255 / width) as u8).wrapping_add(noise >> 3);
            let green = ((y * 255 / height) as u8).wrapping_add(noise >> 4);
            let blue = (((x + y) * 255 / (width + height)) as u8).wrapping_add(noise >> 5);
            pixels.put_pixel(x, y, image::Rgb([red, green, blue]));
        }
    }
    pixels
}

/// Runs `f` in seven timed blocks and returns `(best, median)` milliseconds per
/// call.
fn bench<F: FnMut()>(mut f: F) -> (f64, f64) {
    f();
    let start = Instant::now();
    f();
    let single = start.elapsed().as_secs_f64().max(1e-9);
    let reps = ((0.2 / single) as u64).clamp(1, 4096) as u32;

    let mut per_call = Vec::with_capacity(7);
    for _ in 0..7 {
        let start = Instant::now();
        for _ in 0..reps {
            f();
        }
        per_call.push(start.elapsed().as_secs_f64() * 1000.0 / f64::from(reps));
    }
    per_call.sort_by(f64::total_cmp);
    (per_call[0], per_call[per_call.len() / 2])
}

fn encoded(image: &image::RgbImage, format: ImageFormat) -> Vec<u8> {
    let mut bytes = Vec::new();
    DynamicImage::ImageRgb8(image.clone())
        .write_to(&mut Cursor::new(&mut bytes), format)
        .expect("encode fixture");
    bytes
}

fn encoded_gif(image: &image::RgbImage) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = image::codecs::gif::GifEncoder::new(&mut bytes);
        encoder
            .encode_frame(Frame::new(
                DynamicImage::ImageRgb8(image.clone()).to_rgba8(),
            ))
            .expect("encode gif fixture");
    }
    bytes
}

fn main() {
    let page = source(1600, 1200);
    let png = encoded(&page, ImageFormat::Png);
    let jpeg = encoded(&page, ImageFormat::Jpeg);
    let webp = encoded(&page, ImageFormat::WebP);
    let gif = encoded_gif(&page);
    let cases: [(&str, &[u8]); 4] = [
        ("png", &png),
        ("jpeg", &jpeg),
        ("webp", &webp),
        ("gif", &gif),
    ];

    println!("one 1600x1200 page, median ms per call");
    for (name, bytes) in cases {
        let mib = bytes.len() as f64 / (1024.0 * 1024.0);
        let (best, median) = bench(|| {
            let image = decode(black_box(bytes), false).expect("decode");
            black_box(image);
        });
        let (_, probe_plain) = bench(|| {
            black_box(probe_size(bytes, false).expect("probe"));
        });
        let (_, probe_exif) = bench(|| {
            black_box(probe_size(bytes, true).expect("probe"));
        });
        println!(
            "{name:<5} {mib:5.2} MiB  decode {median:7.2} ms (best {best:.2})  \
             probe {probe_plain:6.3} ms  probe+exif {probe_exif:6.3} ms"
        );
    }

    // Where a page's time actually goes: the app decodes, then caps the long
    // side at 4096 px (no work for this page) and builds an 80 px thumbnail.
    let image: RgbImage = decode(&jpeg, false).expect("decode jpeg");
    let thumb_dims = {
        let scale = 80.0 / f64::from(image.width().max(image.height()));
        (
            ((f64::from(image.width()) * scale).round() as u32).max(1),
            ((f64::from(image.height()) * scale).round() as u32).max(1),
        )
    };
    let (_, thumb_direct) = bench(|| {
        black_box(resize(&image, thumb_dims.0, thumb_dims.1, Filter::Box).expect("thumbnail"));
    });
    let (_, thumb_fast) = bench(|| {
        black_box(thumbnail(&image, thumb_dims.0, thumb_dims.1, Filter::Box).expect("thumbnail"));
    });
    let (_, decode_only) = bench(|| {
        black_box(decode(&jpeg, false).expect("decode"));
    });
    println!(
        "\nper-page flow (jpeg): decode {decode_only:.2} ms + thumbnail to {}x{} {thumb_fast:.2} ms \
         = {:.2} ms",
        thumb_dims.0,
        thumb_dims.1,
        decode_only + thumb_fast
    );
    println!(
        "thumbnail alone to {}x{}: direct resize {thumb_direct:.2} ms, decimate first \
         {thumb_fast:.2} ms ({:.2}x)",
        thumb_dims.0,
        thumb_dims.1,
        thumb_direct / thumb_fast.max(1e-9)
    );
}
