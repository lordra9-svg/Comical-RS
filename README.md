# comical-rs

A behavior-compatible comic viewer for Windows, macOS (pending) and Linux. It reads ZIP,
RAR and 7z archives and is written from a clean-room specification.

## Usage

Run the viewer on a book:

```
cargo run -p comical-app -- target/fixtures/sample.cbz
```

Generate synthetic fixtures and probe them:

```
cargo run -p xtask -- target/fixtures
cargo run -p comical-cli -- probe target/fixtures/sample.cbz
cargo run -p comical-cli -- probe target/fixtures/sample.cbr
cargo run -p comical-cli -- probe target/fixtures/sample.cb7
cargo run -p comical-cli -- extract target/fixtures/sample.cbz 0 target/page.bin
```

Pass `--password <value>` for encrypted ZIP AES-256, RAR data or headers, and
7z AES-256 archives. Passwords are never written to disk. Pass `--natural-sort`
to `probe` or `extract` to order digit runs numerically (`page2` before
`page10`); without it book order is byte order, per the behavior spec.

## Layout

```
spec/                  behavior specification
tests/vectors/         expected outputs, no reference code
docs/                  implementation documentation and maintenance runbook
crates/                product crates
  comical-core/        shared domain types
  comical-archive/     read-only ZIP, RAR, 7z and directory access
  comical-image/       decode, rotation and resampling
  comical-app/         desktop viewer (eframe/egui)
  comical-cli/         probe and extract commands
xtask/                 synthetic archive fixture generator
```

Implementation details, file ownership and the maintenance runbook start at
`docs/README.md`.

## License

MIT for this repository. `rars` and `sevenz-rust` remain Apache-2.0. See
`THIRD_PARTY_LICENSES.md`.
