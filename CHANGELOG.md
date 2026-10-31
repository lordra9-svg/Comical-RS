# Changelog

All notable changes to Comical-RS are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

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
