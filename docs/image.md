# Image pipeline

`comical-image` turns archive bytes into an opaque RGB page, rotates it and
resizes it for the current zoom. It has no archive, UI or OS dependencies.

Source: `crates/comical-image`.

## Public API

```rust
pub struct RgbImage { /* width, height, RGB8 pixels */ }

pub enum Rotation { None, Cw90, Cw180, Cw270 }
pub enum Filter { Box, Bilinear, Bicubic, BSpline, CatmullRom, Lanczos3 }

pub fn decode(bytes: &[u8]) -> Result<RgbImage, ImageError>;
pub fn rotate(image: &RgbImage, rotation: Rotation) -> RgbImage;
pub fn resize(image: &RgbImage, width: u32, height: u32, filter: Filter)
    -> Result<RgbImage, ImageError>;
```

`RgbImage::new` validates dimensions and buffer length. Fields are private, so a
caller cannot construct an image with a mismatched buffer.

`Filter` defaults to `CatmullRom`. `Rotation` defaults to `None`.

## Decode

`decode` accepts PNG, JPEG and GIF only.

- `image::guess_format` identifies the container.
- The header dimensions are read before the full decode.
- `MAX_DECODED_PIXELS` is 100,000,000 pixels, about 286 MiB of RGB output.
- The decoder limits width, height and allocation to the header dimensions and
  the pixel cap.
- `DynamicImage::to_rgb8` drops alpha. Alpha is ignored, not composited.
- GIF uses the first frame.
- Empty, malformed, truncated or unsupported input returns `ImageError`.

Decode does not apply EXIF orientation. That decision is still open. A JPEG with
an EXIF rotation is returned in its stored pixel orientation.

## Rotation

Rotation is a clockwise quarter turn. It is index remapping only, with no
interpolation.

| Rotation | Output size | Source `(x, y)` goes to |
|---|---|---|
| `None` | `w x h` | `(x, y)` |
| `Cw90` | `h x w` | `(h - 1 - y, x)` |
| `Cw180` | `w x h` | `(w - 1 - x, h - 1 - y)` |
| `Cw270` | `h x w` | `(y, w - 1 - x)` |

Rotation belongs here. Landscape detection, spread pairing and screen-side
selection belong to `comical-core` and the future UI. Rotate before asking the
core for orientation.

## Resize

`resize` is a separable two-pass convolution with an 8-bit intermediate buffer.

Pass order:

- Horizontal first when `dst_w * src_h <= dst_h * src_w`.
- Vertical first otherwise.

For each destination index and axis:

- `scale = dst / src`.
- Minification: `fscale = scale`, `support = filter_support / scale`.
- Magnification: `fscale = 1`, `support = filter_support`.
- `center = dst_index / scale`.
- Window: `floor(center - support)` to `ceil(center + support)`, clamped to
  `0..src_len - 1`.
- Weight: `fscale * kernel(fscale * (center - source_index))`.
- Normalize weights to sum 1.
- Accumulate in floating point, round with `+0.5`, truncate, clamp to `0..=255`.

Each pass writes 8-bit RGB, so the second pass sees the rounded intermediate.
The 1.0-scale vectors are not all identity: Bicubic and B-spline still filter.
Do not add an equal-size fast path. The golden tests will fail.

### Filters

| Filter | Support | Kernel |
|---|---:|---|
| Box | 0.5 | 1 when `|x| <= 0.5` |
| Bilinear | 1 | `1 - |x|` when `|x| < 1` |
| Bicubic | 2 | Mitchell-Netravali, B = 1/3, C = 1/3 |
| B-spline | 2 | Cubic B-spline |
| Catmull-Rom | 2 | Cubic, B = 0, C = 1/2 |
| Lanczos3 | 3 | `sinc(x) * sinc(x / 3)` when `|x| < 3` |

Cubic polynomials use nested evaluation. Do not use `pow`. The last bit matters
for the exact hashes.

Lanczos needs `sin(PI * x)`. The implementation does not call `f64::sin`: the
platform libm (glibc, the MSVC UCRT, Apple's libm) disagrees in the last bit for
some arguments, and one flipped `f32` weight changes an output byte, which made
the macOS arm64 job fail while Linux and Windows passed. `resize::sin_pi` uses a
Cody-Waite reduction by `PI/2` plus Taylor polynomials, so every target computes
the same weights. The 42 golden hashes are unchanged by that substitution.

Linear light needs the sRGB transfer curve, which is another `powf`. Those
tables live in `src/srgb.rs` as recorded data: `SRGB_TO_LINEAR` is the
byte-to-linear curve, and `LINEAR_TO_SRGB_THRESHOLDS` holds, for each output
byte, the smallest linear value that rounds to it. Nothing on the resize path
calls a transfer function, so the bytes are the same on every platform and the
42 golden hashes are unchanged.

Keep the coarse back-conversion table. `LINEAR_TO_SRGB_COARSE` answers an
interval of the curve with one lookup, and only the intervals that straddle a
rounding boundary fall through to the threshold search. It is built at startup
from the recorded thresholds, never from `powf`, so it cannot round differently
off this platform. An attempt to answer every channel with the search alone was
correct and deterministic but 1.9x to 4.8x slower end to end on these targets,
because the search lengthens the dependency chain in the pixel loop; with the
coarse table the resize is 6% to 13% faster than 0.2.1. Measure before removing
it (`examples/bench_resize.rs`).

## Golden vectors

File: `tests/vectors/images.json`.

- Source: 17x11 RGB8, explicitly stored as `rgb_hex` with `rgb_sha256`.
- Cases: 42, six filters times seven target sizes.
- Scales: 0.1, 0.25, 0.5, 0.75, 1.0, 1.3 and 2.0.
- Both pass orders and odd dimensions are covered.
- Expected value: SHA-256 of the raw RGB8 output, plus the oracle's output
  bytes in `output_rgb_hex`.

Test: `crates/comical-image/tests/images.rs`. It reconstructs the source,
resizes, hashes and asserts every case. Nothing on the resize path calls the C
library, so the hash reproduces everywhere and is the whole comparison. The test
keeps a fallback: where a hash does not reproduce, it compares `output_rgb_hex`
with a two-step channel tolerance and a 1% differing-channel limit, so a future
platform difference names the case, channel and delta instead of printing two
opaque hashes. `output_rgb_hex` is derived by
`examples/derive_expected_pixels.rs`, which refuses to run unless every oracle
hash reproduces first.

The vectors were recorded from a dirty-side oracle. The harness is not in this
repository. Do not regenerate vectors from the Rust implementation; that would
only test the implementation against itself.

## Errors

| Variant | Meaning |
|---|---|
| `EmptyInput` | No bytes |
| `UnsupportedFormat` | Not PNG, JPEG or GIF |
| `Decode` | Decoder rejected malformed or truncated data |
| `InvalidDimensions` | Zero image side, or unusable header |
| `InvalidTarget` | Zero resize target |
| `BufferLength` | RGB buffer does not match dimensions |
| `TooLarge` | Pixel cap, length overflow or allocation failure |

All errors are returned. Decode and resize do not panic on bad input. A normal
`Vec` allocation in `rotate` can still abort on process-wide OOM.

## Tests

| Test file | Covers |
|---|---|
| `tests/decode.rs` | PNG RGB, PNG alpha, JPEG, GIF first frame, empty, garbage, truncated |
| `tests/images.rs` | All 42 golden filter hashes |
| `tests/transform.rs` | Four rotations, four-turn identity, zero targets, buffer length, zero dimensions |

Run:

```text
cargo test -p comical-image
cargo test -p comical-image --release
```

The golden test runs in both debug and release.

## Benchmarks

Two examples measure the pipeline. They print timings and checksums for a human
to compare two builds, and they are not part of the gate: absolute numbers move
with the machine.

```text
cargo run -p comical-image --release --example bench_decode
cargo run -p comical-image --release --example bench_resize
```

- `bench_decode` encodes one 1600x1200 page to PNG, JPEG, GIF and WebP, then
times `probe_size` with and without EXIF, `decode`, and the per-page flow (decode
plus an 80 px thumbnail).
- `bench_resize` times every filter, downscaling to 1200x900 and upscaling to
  2080x1560, and prints output MPix/s.

Each case runs seven timed blocks of about 200 ms and reports the median and the
best. The checksum is FNV-1a over the output pixels: two builds that print the
same checksums did the same work, so a timing difference is not a different
result.

Recorded on an 8-core Linux VM in release mode, median milliseconds for one
1600x1200 page. Treat it as a regression baseline, not a target.

| Stage | Time |
|---|---:|
| `decode` PNG, JPEG, GIF | 12.6, 12.3, 12.8 |
| `decode` WebP | 27.5 |
| `probe_size`, with or without EXIF | at most 0.16 |
| resize to 1200x900, Box through Lanczos3 | 64 to 104 |
| resize to 2080x1560, Box through Lanczos3 | 116 to 212 |
| 80x60 thumbnail from the decoded page | 8.4 |

The resampler is the expensive stage, not the decoder: one 1.08 MPix result
costs five to eight times the full 1.92 MPix decode. Inside it the fixed
per-channel cost dominates the taps, because Bilinear (two taps) to Lanczos3
(nine taps) is only 1.6x the time, so a narrower kernel buys little. Thumbnails
are charged per page and touch every source pixel.

## Maintenance

### Add a filter

1. Add the variant to `Filter`.
2. Add its support and kernel.
3. Add a vector case for each scale in `images.json` from the oracle.
4. Add a test name mapping and run the golden test.
5. Update this document and the spec.

### Change decode formats

1. Add the feature to the `image` dependency, not all default features.
2. Extend the format allowlist.
3. Add a decode test and an invalid-input test.
4. Update `THIRD_PARTY_LICENSES.md` and this document.
5. Update the archive suffix filter if the new format should be a page.

### Debug a hash mismatch

1. Identify the case name from the assertion.
2. Check the pass order for that target.
3. Check the source window and weight normalization.
4. Check nested kernel evaluation.
5. Check rounding after the first pass.
6. Check the 1.0 case for an accidental identity shortcut.
7. Compare in release mode only after debug passes.

Do not edit the expected hash to make the test pass.

## Known gaps

- No EXIF orientation handling.
- No animated GIF playback; first frame only.
- No WebP, AVIF or TIFF.
- No ICC profile or color-space conversion policy.
- No thumbnail helper yet; the 60/100 px rule is still in the spec.
- No image corpus with real-world files.
- No fuzzing of decoders.
- No zero-copy decode path.
- `rotate` uses an infallible allocation.

See `roadmap.md` for the next slices.
