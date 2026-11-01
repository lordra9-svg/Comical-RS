//! Local throughput harness for the resampling filters.
//!
//! `cargo run -p comical-image --release --example bench_resize` prints the
//! median and best wall time for every [`Filter`] at a downscale and an
//! upscale, plus a checksum of each output so two builds can be compared for
//! identical bytes. It is not part of the test gate: timings move with the
//! machine, and CI asserts the golden hashes instead.

use std::hint::black_box;
use std::time::Instant;

use comical_image::{Filter, RgbImage, resize};

/// Deterministic source page: gradients plus a little noise, so the
/// convolution cannot specialise on a constant row.
fn source(width: u32, height: u32) -> RgbImage {
    let mut state = 0x2545_f491_4f6c_dd1du64;
    let mut pixels = Vec::with_capacity((width as usize) * (height as usize) * 3);
    for y in 0..height {
        for x in 0..width {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let noise = (state >> 56) as u8;
            let red = ((x * 255 / width) as u8).wrapping_add(noise >> 3);
            let green = ((y * 255 / height) as u8).wrapping_add(noise >> 4);
            let blue = (((x + y) * 255 / (width + height)) as u8).wrapping_add(noise >> 5);
            pixels.extend_from_slice(&[red, green, blue]);
        }
    }
    RgbImage::new(width, height, pixels).expect("source image")
}

/// Runs `f` in seven timed blocks and returns `(best, median)` milliseconds per
/// call. A block lasts about 200 ms, so timer noise stays well under the
/// differences worth acting on.
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

/// FNV-1a over the output bytes, so two builds can be compared for equality.
fn checksum(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn main() {
    let src = source(1600, 1200);
    let filters = [
        Filter::Box,
        Filter::Bilinear,
        Filter::Bicubic,
        Filter::BSpline,
        Filter::CatmullRom,
        Filter::Lanczos3,
    ];
    // A reader view caps the long side at 2048 px, so a comic page is usually
    // scaled down to fit; upscaling matters for small pages read full-screen.
    let targets = [(1200, 900, "down 0.75x"), (2080, 1560, "up 1.3x")];

    println!("resize from 1600x1200, median ms, output MPix/s, and output checksum");
    for (width, height, label) in targets {
        let mpix = f64::from(width) * f64::from(height) / 1e6;
        println!("-- {label}: {width}x{height} ({mpix:.2} MPix)");
        for filter in filters {
            let (best, median) = bench(|| {
                let out = resize(&src, width, height, filter).expect("resize");
                black_box(out);
            });
            let out = resize(&src, width, height, filter).expect("resize");
            println!(
                "{:<11} {median:8.2} ms  {:6.1} MPix/s (best {best:.2} ms) sum {:016x}",
                format!("{filter:?}"),
                mpix / (median / 1000.0),
                checksum(out.pixels())
            );
        }
    }
}
