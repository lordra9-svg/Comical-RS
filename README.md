# comical-rs

A behavior-compatible comic viewer for Windows, macOS and Linux. It reads ZIP,
RAR and 7z archives and is written from a clean-room specification.

## Clean-room rule

Read only this repository. Do not open the original Comical source, the
analysis document, the C++ harness, the Bloks memory files, or any notes that
quote or describe the original code. Work from `spec/behavior.md` and
`tests/vectors/` only.

If you encounter forbidden material, stop and report it.

## Status

M1. The workspace is scaffolded. `comical-cli` can list pages in ZIP, RAR and
7z archives or directories and extract one page by index. ZIP supports stored,
deflate and AES-256 entries; RAR 1.4 through 7 is covered, including solid,
encrypted data and encrypted headers; 7z supports LZMA2 and AES-256.
Multi-volume RAR is rejected as unsupported. The UI, spread machine and image
filters are not started yet.

## Usage

Generate synthetic fixtures and probe them:

```
cargo run -p xtask -- target/fixtures
cargo run -p comical-cli -- probe target/fixtures/sample.cbz
cargo run -p comical-cli -- probe target/fixtures/sample.cbr
cargo run -p comical-cli -- probe target/fixtures/sample.cb7
cargo run -p comical-cli -- extract target/fixtures/sample.cbz 0 target/page.bin
```

Pass `--password <value>` for encrypted ZIP AES-256, RAR data or headers, and
7z AES-256 archives. Passwords are never written to disk.

## Layout

```
spec/                  behavior specification
tests/vectors/         expected outputs, no reference code
crates/                product crates
  comical-core/        shared domain types
  comical-archive/     read-only ZIP, RAR, 7z and directory access
  comical-image/       placeholder until the image milestone
  comical-app/         placeholder until the UI milestone
  comical-cli/         probe and extract commands
xtask/                 synthetic archive fixture generator
```

## Toolchain

- Rust 1.99.1 with the GNU host is installed.
- Add `%USERPROFILE%\.cargo\bin` to `PATH` in a new shell if `cargo` is not found.
- CMake and Ninja are optional.
- `cargo-nextest`, `cargo-deny`, `cargo-fuzz` are optional at M0.5.

## Milestones

1. ~~Scaffold the Cargo workspace.~~
2. ~~Implement `comical-cli probe <book>` and `extract` for ZIP, RAR and 7z.~~
3. Image decoding, the spread machine and the UI come next.
4. Add `Clean-Room: spec-v1` to every commit.

## License

MIT for this repository. `rars` and `sevenz-rust` remain Apache-2.0. See
`THIRD_PARTY_LICENSES.md`.
