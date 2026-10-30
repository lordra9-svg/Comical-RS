# Roadmap and known gaps

This document separates implemented code from planned code. Do not treat a
planned item as a contract until it has tests and a spec section.

## Status

| Slice | State | Code |
|---|---|---|
| M0 spec and vectors | Done | `spec/behavior.md`, `tests/vectors/` |
| M1 archive | Done | `comical-archive`, `comical-cli` |
| M2 core layout and zoom | Done | `comical-core` |
| M3 image pipeline | Done | `comical-image` |
| M4–M5 desktop UI | Done | `comical-app` (toolbar, thumbs, rotation, RTL, settings) |
| Memory budgets | Done | `comical-app/src/cache.rs` (byte-budgeted LRU) |
| CI | Done | `.github/workflows/ci.yml`, `scripts/ci.sh` |
| M7 EXIF orientation | Done | `comical-image/src/decode.rs` (nom-exif) |
| M8 Webtoon mode (data model) | Done | `comical-core/src/spread.rs` (`ViewMode::Webtoon`) |
| M9 parallel prefetch | Done | `comical-app/src/app.rs` (rayon prefetch_around) |
| M10 WebP/AVIF decode | Done | `comical-image/Cargo.toml` (image features) |
| M11 fuzz targets | Done | `fuzz/fuzz_targets/` (archive, decode, navigation) |
| Real archive corpus | In progress | `tests/corpus/` Info-ZIP + 7-Zip; RAR writer samples open |
| M12 packaging | In progress | release scripts scaffolded; installers pending |

## M3: image pipeline

Target crate: `comical-image`.

Planned responsibilities:

1. Decode JPEG, PNG and GIF bytes to a predictable pixel buffer.
2. Apply rotation for north, east, south and west.
3. Resample to a target size with the six filters in the behavior spec.
4. Report decode and allocation errors without panicking.
5. Enforce a decoded pixel cap before allocating.

Open implementation choices:

- Use the `image` crate for decode, or separate codec crates.
- Pixel layout: RGB8, RGBA8 or a custom buffer.
- Alpha policy for PNG and GIF.
- Animated GIF: first frame or animation.
- EXIF orientation: honor or ignore.
- Filter kernels: implement from published math, not copied source.

Testing:

- `tests/vectors/images.json` holds 42 oracle-confirmed cases (six filters,
  seven target sizes) and is checked byte for byte on every platform.
- Unit tests cover `sin_pi`/`sinc` and the arithmetic-only Lanczos kernel.
- Decode tests cover every supported format.
- Add a golden test for rotation and for alpha handling.
- Keep image tests outside the core crate.

Do not copy resize code from another project. Do not add a GPL image library.

## M4: desktop UI

Target crate: `comical-app`. The M4 slice is implemented and verified on
Windows. Details: `docs/app.md`.

Implemented:

1. Open a book from a CLI argument or the `rfd` file dialog.
2. Build page orientations from decoded pages, with a two-pass draw so a
   landscape page splits on its first painted frame.
3. Drive `SpreadState` from the keyboard. One-page and two-page modes.
4. Call `compute_zoom_with_neighbors` for Fit, Width, Height, Full and Custom.
5. Cache decoded pages and uploaded textures, keyed by page, part and target
   size. Panning does no decode, resize or upload.
6. Drag-to-pan with clamping, pan reset on page and zoom changes.
7. F11 fullscreen, error display in the canvas, window title from the file name.

Shipped after M4 (see `docs/app.md`):

1. RTL screen mapping and half swap.
2. Per-page rotation for left and right pages.
3. Separate left and right page labels.
4. Scroll semantics and scrollbar thickness tracking.
5. Toolbar and thumbnail browser.
6. Persistent settings.

Shipped after M4, second slice:

1. Wheel scroll and Ctrl+wheel zoom.
2. `natural_sort`, `remember_page` and `filter` settings wired through the
   viewer, the archive layer and the CLI (`--natural-sort`).
3. Drift-free golden image vectors on macOS arm64 (arithmetic `sin` in the
   Lanczos kernel).

Shipped after M4, third slice:

1. Byte-budgeted LRU caches (`src/cache.rs`) driven by `cache_bytes` and
   `resample_bytes`, replacing the fixed ten-page cache.

Still open for later milestones:

1. Context menu.
2. Manual acceptance checklist on macOS and Linux.
3. Webtoon / continuous vertical scroll mode.
4. Apply EXIF orientation when `exif_orientation` setting is on.
5. Parallel page preload with rayon (background prefetch beyond current workers).
6. WebP and AVIF decode support.
7. Fuzz targets for archive open/read paths.
8. Packaging: installers, file associations and icons.
9. RAR writer corpus samples for `tests/corpus/`.
10. ZIP64 and CP437 edge-case fixtures.

Toolkit decision: eframe/egui with the `wgpu` backend, `rfd` for the file
dialog, `lru` for the two caches.

Known divergences from `spec/behavior.md` are listed in `docs/app.md`. Record
them in the port plan before changing either side.

`tests/vectors/ui.json` is the seed for screen mapping. It has two cases for the
first spread, LTR and RTL. Expand it before implementing the UI screen mapping.

## Infrastructure

### CI

Matrix for Windows, macOS and Linux is in `.github/workflows/ci.yml`:

```text
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

`scripts/ci.sh` mirrors the same gate locally. A failure on any OS is a release
blocker until explained.

### Real archive corpus

Synthetic `xtask` fixtures prove code paths. `tests/corpus/` proves
interoperability with Info-ZIP and 7-Zip:

- ZIP: Info-ZIP stored, deflate, ZipCrypto; 7-Zip ZIP deflate; Unicode and deep paths.
- 7z: LZMA, LZMA2, solid LZMA2, AES-256 data, AES-256 headers.
- Truncated ZIP and 7z must fail to open.
- Manifest records page order, sizes and SHA-256 digests.

Still open:

- ZIP64, ZipCrypto edge cases beyond the one fixture, CP437 from a non-Rust writer.
- RAR samples from WinRAR / rarcli (no RAR writer on the corpus host yet).
- 7z BCJ / Delta and very large entries.
- Duplicate names.

Keep copyrighted samples outside the repository. Regenerate with
`./scripts/regen-corpus.sh`.

### Fuzzing

Fuzz the public open and read paths. A panic on malformed input is a bug. Keep
the entry cap small in fuzz targets.

### Packaging

Not started. Needs installers, file associations and icons. Do not start before
the UI and image pipeline are stable.

## M7: EXIF orientation

Target crate: `comical-image`. The `exif_orientation` setting is already parsed
and persisted but not applied.

Planned responsibilities:

1. Parse EXIF orientation tag from JPEG bytes (and PNG tEXt if present).
2. Map orientation values 1–8 to the existing `Rotation` enum or a new transform.
3. Apply the orientation during decode when the setting is on; skip when off.
4. Update `probe_size` to return oriented dimensions when EXIF is honored.
5. Add test vectors for each of the eight EXIF orientations.

Open implementation choices:

- Use the `kamadak-exif` crate or parse the tag manually (it is 12 bytes in a
  known location for JPEG).
- Whether to rotate pixels or just adjust the spread layout.

Testing:

- Unit tests for each orientation value.
- Golden hash vectors for at least one image per orientation.
- Integration test verifying the setting toggle changes output.

## M8: Webtoon / continuous vertical scroll mode

Target crates: `comical-core`, `comical-app`.

Planned responsibilities:

1. Add `ViewMode::Webtoon` variant.
2. Lay out all pages in a single vertical strip with configurable gap.
3. Scroll position replaces page index as the navigation state.
4. Zoom modes apply uniformly to the strip width.
5. Keyboard navigation scrolls by viewport height or to next page boundary.
6. Thumbnail browser highlights the page currently at the top of the viewport.
7. Remember scroll position per book alongside `remember_page`.

Open implementation choices:

- Virtualize rendering (only decode visible pages + margin) or rely on the
  existing byte-budgeted cache.
- Gap size: fixed pixels or configurable setting.
- Whether two-page spreads appear side-by-side in webtoon mode.

Testing:

- Layout unit tests for various page counts and sizes.
- Scroll position persistence round-trip test.
- Visual acceptance on a real webtoon-format archive.

## M9: Parallel page preload with rayon

Target crate: `comical-app`.

Planned responsibilities:

1. Replace or augment the crossbeam background workers with a rayon thread pool.
2. Prefetch decoded pages ahead of the current viewport based on scroll velocity.
3. Respect the existing `cache_bytes` budget; do not prefetch beyond it.
4. Cancel pending prefetched on page turn or zoom change.
5. Measure and document throughput improvement over the current worker model.

Open implementation choices:

- Keep crossbeam for decode completion signaling and use rayon only for CPU work.
- Adaptive prefetch window based on frame time budget.

Testing:

- Benchmark comparing cold-open latency with and without preload.
- Stress test: rapid page turns do not leak memory or stall the UI.

## M10: WebP and AVIF decode support

Target crate: `comical-image`.

Planned responsibilities:

1. Add WebP and AVIF to the supported format list in `decode`.
2. Update `probe_size` to handle both formats.
3. Add corpus fixtures for each format.
4. Update the behavior spec to include them.

Open implementation choices:

- Use the `image` crate's built-in WebP/AVIF support or dedicated crates
  (`webp`, `avif-decode`).
- Alpha handling policy for both formats.

Testing:

- Decode golden vectors for representative samples.
- Corpus entries covering lossy, lossless and animated variants.

## M11: Fuzz targets

Target: workspace-level fuzz directory.

Planned responsibilities:

1. Fuzz `ComicArchive::open` for ZIP, RAR and 7z.
2. Fuzz `decode` and `probe_size` with truncated and malformed input.
3. Fuzz `SpreadState` navigation with arbitrary page counts.
4. Set entry caps to keep runs fast.
5. Integrate into CI as a nightly job.

Testing:

- Seed corpus from existing test vectors and corpus fixtures.
- Document any crashes found and their fixes.

## M12: Packaging and release

Planned responsibilities:

1. Choose a product name.
2. Generate platform icons and file association metadata.
3. Build installers for Windows (MSI/NSIS), macOS (DMG/pkg) and Linux
   (AppImage/deb/rpm).
4. Sign binaries where applicable.
5. Publish release notes and changelog.
6. Independent clean-room review and legal sign-off.

Do not start until M7–M11 are stable and the UI passes manual acceptance on
all three platforms.

## Open decisions

| Decision | Current state | Impact |
|---|---|---|
| UI toolkit | `eframe`/`egui` with `wgpu`, `rfd` dialog | `comical-app` |
| Natural sort | Opt-in setting; byte order is still the default | archive ordering |
| EXIF orientation | Parsed setting, not applied yet | image pipeline |
| WebP / AVIF | Not in v1 | filter and decoder |
| Animated GIF | Not decided | image pipeline |
| Last-page memory | Implemented behind `remember_page` | settings |
| Product name | Undecided | packaging |
| Independent clean-room reviewer | Not named | release |
| Legal review | Not started | release |

## Definition of done for a slice

A slice is done when:

- The behavior spec covers it.
- The public API is documented.
- Unit or integration tests cover the normal and failure paths.
- The full workspace gate passes.
- The code map and relevant doc are updated.
- The commit carries `Clean-Room: spec-v1`.
- Known gaps are listed rather than hidden.

## Risks

| Risk | Effect | Mitigation |
|---|---|---|
| `rars` is young | RAR compatibility gaps | Real corpus, pin version, test all versions |
| `sevenz-rust` is quiet | 7z codec or API gaps | Real corpus, callback-order tests |
| No CI on macOS/Linux | Cross-platform regressions | Add matrix before UI |
| No real corpus | Synthetic-only confidence | Build corpus before release |
| Password not zeroized | Memory exposure | Use a secret wrapper |
| 7z reopen per page | Slow large books | Measure, then cache or optimize |
| Seed vectors not oracle-confirmed | Contract drift | Oracle runner, mark confirmed cases |
| Clean-room is procedural | Legal risk | Independent review and legal advice |
