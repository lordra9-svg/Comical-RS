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

M0.5. Spec and seed vectors exist. The Cargo workspace is not scaffolded yet.

## Layout

```
spec/                  behavior specification
tests/vectors/         expected outputs, no reference code
crates/                product crates (empty)
xtask/                 build and vector tooling (empty)
```

## Toolchain

- Rust stable. On Windows, the GNU host is acceptable if `gcc` is present.
- CMake and Ninja are optional.
- `cargo-nextest`, `cargo-deny`, `cargo-fuzz` are optional at M0.5.

## First tasks

1. Scaffold the Cargo workspace for `comical-core`, `comical-archive`,
   `comical-image`, `comical-app` and `comical-cli`.
2. Implement `comical-cli probe <book>` for ZIP, RAR and 7z.
3. Add `Clean-Room: spec-v1` to every commit.
4. Add the implementer attestation to `PROVENANCE.md` before the first code
   commit.

## License

MIT for this repository. `rars` and `sevenz-rust` remain Apache-2.0. See
`THIRD_PARTY_LICENSES.md`.
