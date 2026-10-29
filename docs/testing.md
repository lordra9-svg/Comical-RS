# Testing

The repository has layered unit, vector, integration and corpus tests, plus one
doctest. A failure should point to one subsystem.

## Commands

```text
cargo test --workspace
cargo test -p comical-core
cargo test -p comical-archive
cargo test -p comical-image
cargo test -p comical-cli
cargo test -p comical-app
cargo test -p comical-archive zip_
cargo test -p comical-core spread_vectors_match -- --nocapture
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

`cargo test --workspace` is the release gate. `fmt` and `clippy` are part of the
same gate.

## Test layers

| Layer | Location | Count | What it proves |
|---|---|---:|---|
| Archive unit | `comical-archive/src/lib.rs` | 2 | Byte sorting, duplicate stability, suffix filter |
| Archive integration | `comical-archive/tests/archive.rs` | 20 | Formats, encryption, solid, caps, errors |
| Archive corpus | `comical-archive/tests/corpus.rs` | 2 | Info-ZIP / 7-Zip fixtures, truncated failures |
| Core vectors | `comical-core/tests/vectors.rs` | 2 | Every spread and zoom JSON case |
| Core generated spreads | `comical-core/tests/spreads_generated.rs` | 6 | N = 1..12 invariants, boundaries, mode switches |
| Core generated zoom | `comical-core/tests/zoom_generated.rs` | 8 | Orientations, modes, quirks, neighbors |
| Image decode | `comical-image/tests/decode.rs` | 7 | PNG, RGBA alpha, JPEG, GIF, bad input |
| Image golden filters | `comical-image/tests/images.rs` | 1 | All 42 oracle hashes |
| Image transforms | `comical-image/tests/transform.rs` | 9 | Rotations, targets, buffer invariants |
| CLI end-to-end | `comical-cli/tests/cli.rs` | 8 | Probe/extract, output format, passwords, errors |
| App unit | `comical-app/src/{app,view}.rs` | 11 | Defaults, navigation, zoom state, orientation rebuild, split size, pan clamp |
| App integration | `comical-app/tests/open_book.rs` | 1 | Opens a directory book, page 0 decodes at natural size |

`comical-app` window creation, keyboard and pointer events, fullscreen and the
native file dialog are manual checks, not automated tests.

## App tests

- Defaults match the behavior spec: two-page, fit width, 100% custom, no book.
- Turn, page step, first and last move the spread and reset pan.
- View-mode switches keep `ZoomOptions::mode` in sync.
- Custom percent clamps to 1 through 200.
- A decoded landscape page rebuilds the spread as its two halves, and a
  landscape right page changes its part without hiding the left page.
- Split size gives the leftover pixel to the right half; cropping keeps both
  halves.
- Two-page content meets at canvas center; pan clamps to content edges.
- `tests/open_book.rs` writes a real 4x3 PNG into a temp directory and checks
  page count and natural size.

Run the viewer manually on Windows:

```text
cargo run -p comical-app -- target/fixtures/sample.cbz
```

Note: `target/fixtures/sample.cbz` uses deliberately invalid image bytes, so it
exercises the per-page error placeholder, not page pixels. Use a real book to
check rendering.

## Archive tests

The integration tests generate archives at run time. No binary fixtures are
committed. This keeps the repository small and avoids shipping comic pages.

Coverage:

- ZIP stored and deflated, exact bytes.
- ZIP AES-256 with no password, wrong password and correct password.
- ZIP CP437 name without the UTF-8 flag.
- RAR versions 1.4, 1.5, 2.0, 2.9, 3.0, 4.0, 5.0 and 7.0.
- Solid RAR5 and RAR7, every page read.
- Encrypted RAR5 and RAR7 data.
- Encrypted RAR5 and RAR7 headers.
- Multi-volume RAR rejected at the first and last part.
- 7z LZMA2 listing and exact bytes.
- 7z AES-256 with no password, wrong password and correct password.
- Directory recursion, relative slash names, sort.
- Out-of-range index.
- Entry size cap before allocation and in every reader.
- Corrupt archives.
- Archives with no images.
- Unsupported extension.

What is not proven: real archives from other writers, unusual codecs, damaged
headers that still parse, very large books, concurrent reads.

## Image tests

`comical-image` has three test files.

`tests/decode.rs` covers:

- PNG RGB exact pixels.
- PNG RGBA with alpha ignored, including zero alpha.
- JPEG dimensions and approximate color.
- GIF first frame only.
- Empty, garbage and truncated input.

`tests/images.rs` is the golden test. It reads
`tests/vectors/images.json`, reconstructs the 17x11 source from `rgb_hex`,
resizes every case and compares SHA-256 of the raw RGB8 output. It asserts all
42 cases and runs in debug and release.

`tests/transform.rs` covers:

- All four rotations with known pixels.
- Four 90-degree turns return the source.
- Zero target width and height.
- Buffer length and zero image dimensions.
- Resize target validation.

What is not proven: real-world JPEG, PNG and GIF files, EXIF orientation,
animated GIF, ICC profiles, color conversion, very large images, fuzzed
decoders.

## CLI tests

`comical-cli/tests/cli.rs` runs the compiled binary. It checks:

- `probe` output is `index<TAB>size<TAB>name`.
- Byte order and suffix filtering.
- `extract` writes exact bytes.
- Out-of-range extraction fails and creates no file.
- Encrypted ZIP: listing works, extraction needs the password.
- Wrong password error text.
- Directory relative paths.
- No pages, unsupported extension and corrupt errors.
- Write failure to a missing parent.
- Deflated entry extraction.

Passwords appear in test arguments. They are test-only values.

## Core vectors

Files:

- `tests/vectors/spreads.json`
- `tests/vectors/zoom.json`

The harness in `crates/comical-core/tests/vectors.rs` parses them with
`serde_json`. Spread cases construct `SpreadState` with `with_spread` when a start
is present, otherwise `new`, apply one operation and compare the whole spread.

Zoom cases build `Size`, `ViewMode`, `ZoomMode` and `ZoomOptions`, pass optional
neighbors, then assert scale within `1e-6` and exact integer dimensions.

### Spread case schema

```json
{
  "name": "unique case name",
  "pages": ["portrait", "landscape"],
  "start": {"left": {"page": 0, "part": "left"},
            "right": {"page": 0, "part": "right"}},
  "op": {"type": "next_turn"},
  "expected": {"left": {"page": 1, "part": "full"},
               "right": {"page": 2, "part": "full"}}
}
```

`start` may be `null`. `op` may be `first`, `last`, `next_turn`, `prev_turn`,
`next_page`, `prev_page`, or `goto` with a `page` field. `expected.right` may be
`null`.

### Zoom case schema

```json
{
  "name": "two-page-width-neighbor-scrollbar",
  "canvas": {"width": 600, "height": 400},
  "scrollbar": 15,
  "mode": "two_page",
  "page": {"width": 300, "height": 400},
  "zoom": "width",
  "zoom_percent": 100,
  "fit_only_oversize": false,
  "neighbors": [{"width": 1400, "height": 1000}],
  "expected": {"scale": 0.9733333333, "width": 292, "height": 389}
}
```

`neighbors` is optional. `zoom` is `fit`, `width`, `height`, `full` or `custom`.
`mode` is `one_page` or `two_page`.

### Image case schema

```json
{
  "source": {
    "width": 17,
    "height": 11,
    "rgb_hex": "row-major RGB8 bytes",
    "rgb_sha256": "source hash"
  },
  "cases": [
    {
      "name": "catmull_rom-0.5",
      "filter": "catmull_rom",
      "target": {"width": 8, "height": 5},
      "output_sha256": "raw RGB8 output hash"
    }
  ]
}
```

`filter` is `box`, `bilinear`, `bicubic`, `bspline`, `catmull_rom` or
`lanczos3`. Hashes are exact. The test reconstructs the source, resizes and
hashes the output. Do not regenerate vectors from the Rust implementation.

## Fixtures

`xtask` writes synthetic archives:

```text
cargo run -p xtask -- target/fixtures
```

It writes:

- `sample.cbz`, `sample.cbr`, `sample.cb7`
- `store.cbz`, encrypted ZIP, encrypted RAR, RAR5, RAR7
- solid RAR5, encrypted RAR5 data, encrypted RAR5 headers
- encrypted 7z, CP437 ZIP, multi-volume RAR parts, corrupt ZIP

`xtask` is a development dependency only. The product crates do not depend on
it.

The CP437 ZIP writer builds the local header, central directory and end record by
hand so the name has no UTF-8 flag. If ZIP parsing changes, this fixture is the
first place to inspect.

## Regression policy

- A fixed bug gets a named vector or a generated test that failed before the fix.
- Keep the vector name descriptive. Prefer the behavior, not the implementation.
- Do not change an expected value to make a test pass. First decide whether the
  code or the contract is wrong.
- Spec changes and vector changes are separate commits from implementation
  changes when practical.
- Every implementation commit carries `Clean-Room: spec-v1`.

## Debugging a failure

### Spread failure

The assertion prints the vector name. Reproduce it by reading the `start`,
`op` and `pages` fields. Check:

1. Is the start spread valid for the book?
2. Does the operation hit a boundary guard?
3. Is the expected right page a full page or a left half?
4. Does the case exercise a penultimate split page?

Add a focused test in `spreads_generated.rs` if the vector is not enough.

### Zoom failure

The assertion prints the vector name and scale. Check:

1. Mode and page orientation.
2. `fit_only_oversize` and `is_oversized`.
3. Which branch of fit width or fit height runs.
4. Whether a neighbor forces a scrollbar.
5. Truncation before comparison.
6. Square-page wide vs portrait behavior.

Use exact numbers. A one-pixel difference is a real contract difference.

### Image failure

The assertion prints the case name. Check:

1. Pass order for the target size.
2. Source window and weight normalization.
3. Nested cubic evaluation and the B-spline basis.
4. Rounding after the first pass.
5. The 1.0 case: do not shortcut equal dimensions.
6. Alpha handling or GIF frame selection for decode tests.

A one-byte difference is a real contract difference. Run debug and release.

### Archive failure

Check:

1. Extension mapping and `is_dir`.
2. Page filter and byte sort.
3. Declared size vs actual size.
4. Password path.
5. Reader error mapping.
6. For 7z, callback order and target name.

### CLI failure

Run the binary directly to see stderr. The tests compare exact output for
`probe`; a formatting change will fail several tests.

## Real-writer corpus

`crates/comical-archive/tests/corpus.rs` reads `tests/corpus/manifest.json` and
the committed Info-ZIP / 7-Zip fixtures under `tests/corpus/fixtures/`. It checks
page order and exact extracted bytes (and SHA-256). Truncated fixtures must fail
to open. Regenerate with `./scripts/regen-corpus.sh` when the writers or page
stubs change. See `tests/corpus/README.md`.

## CI

`.github/workflows/ci.yml` runs on Windows, macOS and Linux:

```text
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

The workflow uses `actions/checkout@v7` and `Swatinem/rust-cache@v2`, both of
which run on the Node 24 action runtime; `checkout@v4` is the last node20
release and triggers a deprecation warning. Superseded runs on the same ref are
cancelled, and the token is limited to `contents: read`. There is no JavaScript
in the product, so Node exists only for the actions themselves.

`scripts/ci.sh` is the local mirror. Keep the archive and corpus tests in the
matrix; they are pure Rust and should run on all three platforms.

Golden image hashes are expected to be bit-identical everywhere. The Lanczos
kernel must not call the platform `f64::sin`; see `docs/image.md`. A failure on
one OS only is a determinism bug, not a flaky test.
