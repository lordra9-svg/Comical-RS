# Roadmap and known gaps

This document separates implemented code from planned code. Do not treat a
planned item as a contract until it has tests and a spec section.

## Status

| Slice | State | Code |
|---|---|---|
| M0 spec and vectors | Done | `spec/behavior.md`, `tests/vectors/` |
| M1 archive | Done on Windows | `comical-archive`, `comical-cli` |
| M2 core layout and zoom | Done | `comical-core` |
| M3 image pipeline | Done on Windows | `comical-image` |
| M4 desktop UI | Done on Windows (M4 scope) | `comical-app` |
| CI | Not started | no workflow files |
| Real archive corpus | Not started | synthetic fixtures only |
| Packaging | Not started | no installers or associations |

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

- `tests/vectors/images.json` is empty. Fill it with input hashes, filter,
  target size and output hashes.
- Add unit tests for each filter on small synthetic pixels.
- Add decode tests for each format.
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

Still to do in M5 and later:

1. Map book order to screen order for RTL, including the half swap.
2. Rotation for the left and right page.
3. Separate left and right page labels.
4. Scrollbars, key scrolling and arrow-key scroll semantics.
5. Toolbar, context menu, wheel and Ctrl+wheel input.
6. Thumbnail browser.
7. Settings, password prompt and last-page memory.
8. macOS and Linux runs, plus the manual acceptance checklist on each.

Toolkit decision: eframe/egui with the `wgpu` backend, `rfd` for the file
dialog, `lru` for the two caches.

Known divergences from `spec/behavior.md` are listed in `docs/app.md`. Record
them in the port plan before changing either side.

`tests/vectors/ui.json` is the seed for screen mapping. It has two cases for the
first spread, LTR and RTL. Expand it before implementing the UI screen mapping.

## Infrastructure

### CI

Add a matrix for Windows, macOS and Linux:

```text
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Archive tests are pure Rust and should run everywhere. A failure on macOS or
Linux is a release blocker until explained.

### Real archive corpus

Synthetic fixtures prove code paths. They do not prove interoperability.

Build a corpus with:

- ZIP from several writers: stored, deflate, ZIP64, ZipCrypto, AES-256, CP437.
- RAR 1.4 through 7: solid, encrypted data, encrypted headers, split volumes.
- 7z: LZMA, LZMA2, BCJ, Delta, solid, AES-256, header encryption.
- Corrupt and truncated variants.
- Duplicate names, deep paths, Unicode names and very large entries.

Keep copyrighted samples outside the repository. Record hashes and expected page
lists. Run list and exact-byte checks.

### Fuzzing

Fuzz the public open and read paths. A panic on malformed input is a bug. Keep
the entry cap small in fuzz targets.

### Packaging

Not started. Needs installers, file associations and icons. Do not start before
the UI and image pipeline are stable.

## Open decisions

| Decision | Current state | Impact |
|---|---|---|
| UI toolkit | `eframe`/`egui` with `wgpu`, `rfd` dialog | `comical-app` |
| Natural sort | Off, byte sort only | archive ordering |
| EXIF orientation | Not decided | image pipeline |
| WebP / AVIF | Not in v1 | filter and decoder |
| Animated GIF | Not decided | image pipeline |
| Last-page memory | Not decided | settings |
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
