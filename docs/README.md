# Implementation documentation

This directory describes the code as it exists today. It is separate from
`spec/behavior.md`, which is the behavior contract.

Use these documents when you need to change, debug or review the implementation,
or when you need to know which file owns a behavior.

## Source of truth

| Question | Authoritative source |
|---|---|
| What should the viewer do? | `spec/behavior.md` |
| What does the code do now? | The code and these documents |
| What are the expected spread and zoom outputs? | `tests/vectors/` |
| What are the expected image filter outputs? | `tests/vectors/images.json` |
| What is allowed in this repository? | `CLEANROOM.md`, `PROVENANCE.md` |
| What licenses apply? | `LICENSE`, `THIRD_PARTY_LICENSES.md` |

If a document and the code disagree, treat the code as current behavior and the
behavior spec as the intended behavior. Record the difference before changing
either one.

## Reading order

1. `architecture.md`: crate boundaries, data flow and design rules.
2. `code-map.md`: every file, its symbols and the tests that protect it.
3. `core.md`: spread state and zoom math in detail.
4. `archive.md`: ZIP, RAR, 7z and directory readers in detail.
5. `image.md`: decode, rotation, filters and golden hashes.
6. `app.md`: desktop viewer state, frame sequence, caches and divergences.
7. `testing.md`: test layers, fixtures and vectors.
8. `runbook.md`: build, diagnose, change and release procedures.
9. `roadmap.md`: placeholders, planned milestones and known gaps.

## Status

The repository contains the M1 archive slice, the M2 core slice, the M3 image
slice and the M4 viewer slice. The viewer runs on Windows and covers the M4
scope. Toolbar, thumbnails, rotation, RTL, settings and CI are still to come.

## Documentation rules

- Write about this repository only. Do not add notes from forbidden material.
- Use the public names in the code, not names from any other implementation.
- Update the code map when a file, public type or ownership boundary changes.
- Update the runbook when a command or dependency changes.
- Keep behavior details in `spec/behavior.md`, not duplicated here. Link to it.
- Mark planned work as planned. Do not describe it as implemented.

## Quick commands

```text
cargo test --workspace
cargo test -p comical-image
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p xtask -- target/fixtures
cargo run -p comical-cli -- probe target/fixtures/sample.cbz
cargo run -p comical-cli -- extract target/fixtures/sample.cbz 0 target/page.bin
cargo run -p comical-app -- target/fixtures/sample.cbz
```

If `cargo` is not found, prepend `%USERPROFILE%\.cargo\bin` to `PATH`.
