# Architecture

This document describes the Comical-RS codebase structure, conventions and
constraints. Feed it to any LLM co-developer at the start of every session so
generated code stays consistent with the existing design.

## Crate layout

```text
crates/
├── comical-core      # Pure logic: SpreadState, ViewMode, zoom math, page orientation
├── comical-archive   # Archive I/O: ZIP, RAR, 7z open/read/list; password support
├── comical-image     # Decode, probe_size, EXIF rotation, resample, rotate
├── comical-app       # egui/wgpu desktop UI, settings, caches, background workers
└── comical-cli       # Headless CLI for probe, extract and metadata inspection
```

Additional top-level directories:

- `fuzz/` — cargo-fuzz targets (excluded from workspace)
- `xtask/` — Build automation and corpus regeneration
- `spec/` — Behavior specification (`behavior.md`)
- `tests/vectors/` — Golden image and UI test vectors
- `tests/corpus/` — Real-world archive fixtures (Info-ZIP, 7-Zip)
- `docs/` — Roadmap, app notes, provenance
- `scripts/` — CI gate, corpus regeneration shell scripts

## Key design decisions

| Decision | Choice | Rationale |
|---|---|---|
| UI toolkit | eframe/egui + wgpu backend | Cross-platform, GPU-accelerated, no native deps |
| File dialog | rfd | Native dialogs on all platforms |
| Archive formats | zip, rars, sevenz-rust | Pure Rust, no system libs, clean-room safe |
| Image decode | image crate (png, jpeg, gif, webp, avif) | Single dependency, format detection built in |
| EXIF parsing | nom-exif | Pure Rust, actively maintained, zero-copy |
| Parallelism | rayon for CPU prefetch, crossbeam for worker signaling | rayon = data parallelism; crossbeam = bounded channels |
| Caching | Byte-budgeted LRU (cache_bytes, resample_bytes) | Predictable memory, no unbounded growth |
| Natural sort | Opt-in setting, byte-order default | Backward compatible, user-controlled |
| Settings persistence | Versioned TOML in platform config dir | Human-readable, forward-compatible |
| Fuzzing | cargo-fuzz with libfuzzer-sys | Standard Rust fuzzing infra, nightly-only |

## Coding conventions

1. **Edition**: All crates use Rust 2024 edition, resolver v3.
2. **Error handling**: Use `thiserror` for public error types. Never panic on
   malformed input; return `Result` or a sentinel value.
3. **Public API**: Document every `pub` item. Prefer builder patterns over
   positional arguments for functions with >3 parameters.
4. **Tests**: Every new feature needs unit tests. Image pipeline changes need
   golden vector updates in `tests/vectors/`. Archive changes need corpus
   entries in `tests/corpus/`.
5. **Clean-room**: Every commit must carry `Clean-Room: spec-v1` trailer.
   Do not copy code from GPL/LGPL projects. Do not reverse-engineer
   proprietary codecs.
6. **Formatting**: Run `cargo fmt --all` before committing. CI enforces this.
7. **Linting**: Run `cargo clippy --all-targets -- -D warnings`. Zero warnings
   policy; use `#[expect(...)]` with justification if suppression is needed.
8. **Dependencies**: Prefer pure-Rust crates. Avoid C/system dependencies.
   Pin versions in Cargo.lock; do not use wildcard versions.
9. **Performance**: Profile before optimizing. Document benchmarks when
   adding performance-sensitive code. Respect byte budgets in cache code.
10. **Platform parity**: Test on Windows, macOS and Linux. CI matrix covers
    all three. Manual acceptance required before release.

## State management

- `App` owns global state: settings, current book, spread state, textures.
- `OpenedBook` owns per-book state: archive handle, page caches, decode
  channels, EXIF orientation flag. Threaded through constructor.
- `SpreadState` is pure logic in `comical-core`; no I/O, no allocation beyond
  the page list. Drives navigation for all view modes.
- Background workers use crossbeam channels for completion signaling. Rayon
  handles CPU-bound prefetch independently.

## Adding a new feature checklist

1. Read this document and `spec/behavior.md`.
2. Identify which crate(s) are affected.
3. Write tests first (unit + golden vectors where applicable).
4. Implement with `Clean-Room: spec-v1` discipline.
5. Run `cargo fmt --all && cargo clippy --all-targets -- -D warnings`.
6. Run `cargo test --all`.
7. Update `docs/roadmap.md` status table.
8. Commit with conventional message + clean-room trailer.
9. Push and verify CI passes on all platforms.