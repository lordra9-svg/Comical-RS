# Changelog

All notable changes to Comical-RS are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added
- M12 packaging scaffolding: release build script, CHANGELOG, cargo-deb/cargo-wix/cargo-bundle configs
- ARCHITECTURE.md and .cursorrules for LLM co-development alignment
- M11 fuzz targets for archive open, image decode and spread navigation
- M10 WebP and AVIF decode support via image crate features
- M9 parallel page prefetch with rayon (prefetch_around)
- M8 ViewMode::Webtoon data model, navigation, settings serialization and UI radio button
- M7 EXIF orientation applied during decode and probe_size via nom-exif
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
- Lanczos kernel computed with arithmetic sin_pi, not platform libm
- Solid 7z seek handling for compressed entries
- Common wrapper directory stripped from archive entry paths

### Known gaps
- Webtoon continuous scroll renderer not yet implemented (data model only)
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