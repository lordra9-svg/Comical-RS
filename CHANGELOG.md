# Changelog

All notable changes to Comical-RS are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added
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
