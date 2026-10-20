# Testing

The repository has 46 tests at the M2 slice. Tests are layered so a failure
points to one subsystem.

## Commands

```text
cargo test --workspace
cargo test -p comical-core
cargo test -p comical-archive
cargo test -p comical-cli
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
| Core vectors | `comical-core/tests/vectors.rs` | 2 | Every spread and zoom JSON case |
| Core generated spreads | `comical-core/tests/spreads_generated.rs` | 6 | N = 1..12 invariants, boundaries, mode switches |
| Core generated zoom | `comical-core/tests/zoom_generated.rs` | 8 | Orientations, modes, quirks, neighbors |
| CLI end-to-end | `comical-cli/tests/cli.rs` | 8 | Probe/extract, output format, passwords, errors |

The `comical-image` and `comical-app` crates have no tests because they have no
implementation yet.

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

## CI

There is no CI yet. The current tests have only run on Windows with the GNU
toolchain. macOS and Linux are not verified. Add a matrix that runs:

```text
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Keep the archive tests in the matrix. They are pure Rust and should run on all
three platforms.
