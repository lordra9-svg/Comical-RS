# Changelog

All notable changes to Comical-RS are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Changed
- Both resampling passes walk rows instead of pixels. The horizontal pass
  converts one source row to linear light and reads every window out of it, and
  the vertical pass accumulates a whole destination row while the taps walk
  whole source rows, so the inner loop stays inside one row. Each output
  channel still starts at zero and adds its taps in the same order and scale 1.0
  still takes the full path, so the 42 golden hashes and every benchmark
  checksum are unchanged. The reading view gets faster: Catmull-Rom from
  1600x1200 to 1200x900 drops from 86.5 ms to 70.4 ms, and the `bench_resize`
  matrix improves 13% to 29% at 1.0x, 0.75x and 1.3x.

## [0.2.3] — 2026-11-02

### Changed
- `comical-image` has a `thumbnail` function that averages blocks of the source
  down to twice the target, in linear light, before running the same filter. A
  large minification widens the filter window instead of narrowing it
  (`support = 3 / scale`), so an 80 px thumbnail of a 1600 px page read every
  source pixel sixteen times over: 8.4 ms for a direct `resize`, 2.7 ms now, and
  the browser's per-page flow from 20.0 ms to 13.7 ms. `resize` is untouched and
  still produces the golden vectors; the pre-pass rounds once, so `thumbnail` is
  close to `resize` but not byte identical and stays a separate function.

### Added
- `comical-image/examples/bench_decode.rs` and `bench_resize.rs` measure the
  decode path and every resampling filter and print a checksum of each output,
  so a change can be compared against the previous build. `docs/image.md`
  records the recorded baseline and how to read it.
- `bench_decode` now sweeps every filter for the 80 px strip, direct `resize`
  against decimated `thumbnail` plus the largest channel difference between the
  two, and splits the JPEG decode into the `image` crate's own work and this
  crate's RGB8 conversion. A sharper strip costs little in the decimated path
  (2.6 ms for Box, 3.1 ms for Lanczos3, against 8.4 ms to 46.1 ms direct), and
  the split puts 10.7 ms of the 11.0 ms decode inside the decoder. `bench_resize`
  gained a same-size case that isolates the fixed per-pixel cost: 84 ms for Box
  on a 1600x1200 page, where the taps cannot explain it. The reading view's
  resize to 1200x900 (64 to 106 ms) is therefore the largest remaining per-page
  cost, ahead of the 11 ms decode.

### Fixed
- 0.2.2 answered every channel of the linear-to-sRGB pass with a binary search
  over the recorded thresholds. That is correct and deterministic, but the
  search lengthens the dependency chain inside the pixel loop: a 1600x1200 to
  2080x1560 Catmull-Rom resize went from 194 ms to 592 ms, and the same-size box
  case from 83 ms to 400 ms. The coarse table is back, derived from the recorded
  thresholds instead of `powf`, so the bytes are unchanged and the resize is now
  6% to 13% faster than 0.2.1 (77 ms, 73 ms, 172 ms and 76 ms for those four
  benchmarks).

## [0.2.2] — 2026-11-01

### Changed
- The sRGB transfer tables are recorded data in `comical-image/src/srgb.rs`
  instead of values computed from `powf` at startup. `powf` is the platform
  libm, so one table entry differing in its last bit could push a channel across
  a rounding boundary and change an output byte. The linear-to-sRGB pass is now
  a binary search over those thresholds, which removes the 64 KiB coarse lookup
  table and never calls a transfer function. The 42 golden hashes are unchanged.
- The golden image vectors record the oracle's output bytes for every case. The
  SHA-256 stays the primary comparison; where a hash does not reproduce, the
  test compares those bytes with a two-step channel tolerance and a 1% differing
  channel limit, and names the case, channel and delta when it fails.

### Fixed
- `xtask`'s corpus manifest test no longer fails on a Windows checkout. Git
  rewrote the committed JSON to CRLF there, so the regenerated file never
  matched; `.gitattributes` now pins the manifest to LF and keeps the corpus
  payloads binary.
- macOS CI passes again: `lanczos3-2.0` was the one golden case where a platform
  `powf` difference reached an output byte.

## [0.2.1] — 2026-11-01

### Added
- Package verification reads the MSI's database instead of trusting its
  container. The embedded cabinet payload must be the executable the same run
  built, the table streams must carry the `.cbz`/`.cbr`/`.cb7` associations,
  the `[#ComicalEXE],0` icon reference and the Start Menu shortcut, and the
  product version must match the artifact's file name, so an edit that drops a
  component fails before upload.
- CI builds and verifies a real release. A `Release packaging` job installs
  `wixl` and mingw-w64, runs `xtask release 0.0.0`, requires the MSI to exist and
  then runs `xtask verify`, on every push to master.
- `cargo run -p xtask -- verify [directory]` checks a finished release
  directory before it is published: the checksum manifest and its coverage, the
  Linux tarball's member, mode, ownership and bytes, the Debian and MSI
  containers, and the Windows executable's PE machine type, GUI subsystem,
  `.rsrc` section and required imports. `xtask release` now prints it as the
  first next step.

### Fixed
- The Windows build actually runs on the wgpu renderer. `eframe` was built with
  its default features, so the glow backend was still compiled in and eframe
  selected OpenGL at runtime; glow is now turned off and the renderer is named
  explicitly. The executable also carries an icon, version information and an
  application manifest (asInvoker, per-monitor DPI, Windows 10/11, long paths),
  where it previously had no `.rsrc` section at all.
- GUI builds no longer fail silently. Panics and fatal startup errors are
  appended to `comical.log` in the platform data directory, a fatal error on
  Windows also opens a message box naming that file, and the log records the
  wgpu backend and adapter that came up, which is the only way to tell DX12
  from Vulkan or GLES in a release build.

### Changed
- The MSI no longer needs a Windows host. `xtask release` builds it with
  `wixl` from msitools when that is installed and falls back to `cargo-wix`,
  passing both the binary path and the version of the current run. The WiX
  source no longer uses preprocessor instructions, which wixl rejects, and the
  installer now shows the MIT license from `License.rtf`.
- The MSI packages the executable the release run actually built. The WiX
  source takes its `Source` from a `BinaryPath` variable that `xtask release`
  fills in, and the MSI step now also runs after a Linux cross build instead of
  only on a Windows host.
- Repository tooling is Rust only. The `scripts/` shell scripts became the
  `ci`, `release` and `regen-corpus` subcommands of the `xtask` crate, which
  removes the bash and Python dependency and behaves the same on Windows,
  macOS and Linux. `cargo run -p xtask -- fixtures <dir>` still writes the
  synthetic fixtures.

## [0.2.0] — 2026-10-31

Superseded by 0.2.1: the artifacts uploaded for 0.2.0 predate the renderer and
packaging fixes below, so download 0.2.1 instead.

### Added
- M12 packaging scaffolding: release build script, CHANGELOG, cargo-deb/cargo-wix/cargo-bundle configs
- ARCHITECTURE.md and .cursorrules for LLM co-development alignment
- M11 fuzz targets for archive open, image decode and spread navigation
- M10 WebP decode support via image crate features
- M9 parallel page prefetch with rayon (prefetch_around)
- M8 ViewMode::Webtoon data model, navigation, settings serialization and UI radio button
- M7 EXIF orientation applied during decode and probe_size via nom-exif
- Webtoon continuous vertical scroll renderer
- Byte-budgeted LRU caches replacing fixed ten-page cache
- Background decode and thumbnail workers for responsive UI
- Password prompt and retry for encrypted archives
- Info-ZIP and 7-Zip real archive corpus fixtures
- Toolbar and thumbnail browser panels
- Persistent versioned TOML settings in platform config dir
- Per-page rotation with keyboard controls
- Scroll semantics per spec 5.6
- Separate page labels for two-page spreads
- RTL screen mapping and half swap
- Cross-platform GitHub Actions CI workflow

### Fixed
- Keyboard shortcuts no longer fire while a text field has focus. Typing a
  password that contained a shortcut letter used to navigate, rotate pages and
  flip the reading direction at the same time.
- Shift+Arrow scrolls horizontally again. The Shift+Left/Right rotation aliases
  shadowed the documented scroll path, so those branches were unreachable.
- EXIF orientation can be toggled from the settings window and applies to the
  open book immediately, instead of being a persisted key with no UI control
  and no effect until the book was reopened.
- A poisoned archive mutex no longer panics the viewer; the archive handle is
  still usable after a background worker panics.
- `scripts/release.sh` reads the version from the workspace manifest, which is
  the only place it exists now that crates inherit it, and refuses to run when
  it cannot.
- Release packaging also emits a Linux tarball and `SHA256SUMS.txt` so the
  upload can be verified.
- The Windows executable links as a GUI subsystem program, so starting the
  viewer no longer opens an extra console window behind it.
- `scripts/release.sh` cross-compiles the Windows executable on Linux when a
  mingw-w64 toolchain and the `x86_64-pc-windows-gnu` target are available,
  instead of only ever building it on Windows.

### Known gaps
- RAR writer corpus samples missing (no WinRAR/rarcli on corpus host)
- ZIP64 and CP437 edge-case fixtures not yet added
- macOS/Linux manual acceptance checklist not completed
- Independent clean-room reviewer not named
- Legal review not started
- Binary signing not configured
- File association installers not wired into release script

## [0.1.0] — Initial milestone set

M0 through M6 shipped as the foundational codebase. See git history for
individual commits.
