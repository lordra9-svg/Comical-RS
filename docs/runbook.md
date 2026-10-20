# Maintenance runbook

Use this when building, debugging, changing or releasing the viewer.

## Toolchain

Known working setup:

- Windows 10/11.
- Rust 1.99.1, host `stable-x86_64-pc-windows-gnu`.
- `gcc` 16.2.0 on `PATH`.
- `%USERPROFILE%\.cargo\bin` on `PATH`.
- MSVC Build Tools are installed but not used by the GNU host.

Check:

```text
cargo --version
rustc -vV
gcc --version
```

If `cargo` is missing in a new shell, prepend the cargo bin directory.

The workspace declares `rust-version = "1.89"` and edition 2024. The installed
toolchain is newer. Do not lower the declared version without checking every
crate.

## Build and test

```text
cargo build --workspace
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Focused:

```text
cargo test -p comical-core
cargo test -p comical-archive
cargo test -p comical-image
cargo test -p comical-cli
cargo test -p comical-image --release
```

Run the CLI:

```text
cargo run -p comical-cli -- probe path\to\book.cbz
cargo run -p comical-cli -- extract path\to\book.cbz 0 out.bin
cargo run -p comical-cli -- extract path\to\book.cbr 0 out.bin --password secret
```

Generate fixtures:

```text
cargo run -p xtask -- target/fixtures
```

## Diagnose common failures

| Symptom | Likely cause | Check | Fix location |
|---|---|---|---|
| `Cannot open` | Extension not mapped, or path not a directory | `format_for`, file name | `comical-archive/src/lib.rs` |
| `This archive does not contain any pages` | No image suffixes, or wrong filter | `is_image_name`, entry names | `comical-archive/src/lib.rs` |
| Pages in wrong order | Sort changed, or natural sort added | `select_pages` | `comical-archive/src/lib.rs` |
| Page bytes wrong | Wrong source index, callback order, name decode | reader `read_page` | format module |
| `password` on listing | Headers are encrypted | format behavior | expected |
| `wrong password` | Password mismatch or corrupt data | password path | format module |
| `entry ... above ... limit` | Declared size or actual read above cap | page size, cap | reader `read_page` |
| `archive is corrupt` on a real book | Unsupported variant or parser gap | reproduce with a minimal case | format module |
| `multi-volume RAR` | Volume set | expected | no fix in v1 |
| Spread skips a page | Turn boundary guard | vector case | `comical-core/src/spread.rs` |
| Previous page at page 0 resets | Missing page-zero guard | `prev_page_two_page` | `comical-core/src/spread.rs` |
| Zoom off by one | Truncation or integer division | `scaled_dimension`, fit base | `comical-core/src/zoom.rs` |
| Scrollbar appears when not needed | Neighbor fallback too aggressive | neighbor tests | `comical-core/src/zoom.rs` |
| Image hash mismatch | Pass order, window, nested kernel or rounding | `tests/images.rs` case | `comical-image/src/resize.rs` |
| `unsupported image format` | Bytes are not PNG, JPEG or GIF | `guess_format` allowlist | `comical-image/src/decode.rs` |
| `image is too large` | Header above 100M pixels or allocation failed | header dimensions, cap | `comical-image/src/decode.rs` |
| Rotation wrong | Index mapping or direction | rotation test | `comical-image/src/rotate.rs` |
| CLI output changed | Format string | `probe` | `comical-cli/src/main.rs` |

Always reproduce first. Add a failing test before changing behavior.

## Add a page suffix

1. Update `is_image_name` in `comical-archive/src/lib.rs`.
2. Update the suffix unit test.
3. Update `spec/behavior.md` and `docs/archive.md`.
4. Add one archive integration test that lists the new suffix and one that
   rejects a lookalike (for example `.webp.txt`).
5. Run the full gate.

Remember: the filter is a suffix check, not content sniffing. The image pipeline
must still reject undecodable bytes.

## Add an archive format

Follow `docs/archive.md`, section "Adding a new archive format". In short:

1. `ArchiveFormat` variant.
2. Extension mapping.
3. Module with `open` and `ComicArchive`.
4. Candidate filtering, `select_pages`, `NoPages`.
5. Bounded `read_page`.
6. Error mapping.
7. Minimal dependency features.
8. License and docs updates.
9. Tests for list, exact bytes, cap, corruption, password and unsupported
   features.

Do not extract to a temporary directory. Do not add a writer to the product
crate.

## Change the entry cap

Default: `DEFAULT_MAX_ENTRY_BYTES = 512 * 1024 * 1024` in
`comical-archive/src/lib.rs`.

If the cap changes:

- Update the constant and its documentation.
- Update CLI behavior if it should accept a flag.
- Keep the pre-allocation check and the observed-size check.
- Test a declared size over the cap and an actual read over the cap.
- Consider a configurable limit for the UI later.

## Change the image pipeline

Follow `docs/image.md`.

For decode:

1. Keep the format allowlist in `decode`.
2. Add the smallest `image` feature needed.
3. Check header dimensions and the pixel cap before decode.
4. Add a decode test and a bad-input test.
5. Update `THIRD_PARTY_LICENSES.md`.

For a filter:

1. Add the variant and support.
2. Implement the kernel with nested evaluation.
3. Add oracle vectors for every scale.
4. Run the golden test in debug and release.
5. Do not shortcut equal source and target sizes.

For rotation:

1. Keep the mapping in `rotate`.
2. Test all four states and four quarter turns.
3. Update orientation tests if the direction convention changes.

## Password handling

- Passwords are never persisted.
- `OpenOptions` borrows; books own a `String`.
- The CLI passes `--password`.
- Do not print the password in errors or logs.
- Do not add it to settings.
- If adding zeroization, use a wrapper such as `secrecy`, keep the public API
  stable, and add tests for required, wrong and correct passwords.

## Upgrade an archive dependency

For `zip`:

1. Read the changelog for `ZipArchive`, `ZipReadOptions`, name decoding and AES.
2. Keep the feature set minimal: deflate via zlib-rs, AES.
3. Run ZIP store, deflate, AES, CP437 and corrupt tests.
4. Check the license.

For `rars`:

1. Check `ArchiveReader`, `ArchiveReadOptions`, `read_member_at_with_options`,
   volume flags and error kinds.
2. Keep the encryption feature.
3. Run RAR 1.4 through 7, solid, data encryption, header encryption and
   multi-volume tests.
4. Check the license.

For `sevenz-rust`:

1. Check `SevenZReader`, `for_each_entries` order, password errors and
   `SevenZMethod`.
2. Keep `aes256` and `compress` unless the crate changes the feature dependency.
3. Run 7z list, exact bytes, AES and corrupt tests.
4. Re-check callback order and the name guard.
5. Check the license.

For `image`:

1. Keep `default-features = false` and only `png`, `jpeg`, `gif`.
2. Check `ImageReader`, `Limits`, format detection and `to_rgb8` alpha handling.
3. Run the 42 golden hashes in debug and release.
4. Check the pixel cap and allocation limit still apply before decode.
5. Check `image` and its transitive licenses.
6. Regenerate the license inventory with `cargo metadata` or `cargo tree`.

After any upgrade, run the full workspace gate and a real-world corpus when one
exists. Record the version change in `Cargo.lock` and `THIRD_PARTY_LICENSES.md`
if the license or dependency set changed.

## Cross-platform notes

- Book names use `/`, even on Windows. Directory and RAR readers normalize.
- Page sorting is byte order, not locale order.
- File paths in errors use the platform display form.
- The CLI writes bytes exactly; do not open output in text mode.
- 7z and ZIP entry names are Unicode or decoded by the crate.
- Symlinks in directory books are skipped.
- No macOS or Linux build has been verified yet.

When adding CI, run the archive tests on all three platforms. Do not assume the
GNU-specific behavior is portable.

## Provenance and licensing

- New code is MIT.
- `rars`, `sevenz-rust` and `lzma-rust` are Apache-2.0.
- `zlib-rs` is Zlib.
- `r-efi` is triple-licensed MIT / Apache-2.0 / LGPL-2.1-or-later; the project
  elects MIT or Apache-2.0.
- No GPL, AGPL, LGPL-only or unRAR dependency.
- Every commit carries `Clean-Room: spec-v1`.
- Add an access-log line to `PROVENANCE.md` when a role does substantive work.
- Do not put original source, analysis notes, C++ harness code or source-derived
  identifiers in this repository.

Before release, generate a full dependency license report with `cargo-about` or
`cargo-deny`, vendor the required notices, and have legal review the result.

## Release checklist

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo test --workspace`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Windows, macOS and Linux CI green
- [ ] Real-world ZIP, RAR and 7z corpus passes list and exact-byte checks
- [ ] Encrypted ZIP, RAR and 7z samples pass
- [ ] Multi-volume RAR fails with the documented error
- [ ] Image decode and filter hashes pass (after M3)
- [ ] UI smoke test opens a CBZ (after M4)
- [ ] License report and notices updated
- [ ] `PROVENANCE.md` current
- [ ] Independent clean-room review complete
- [ ] Legal review complete
- [ ] Product name, icons and repository URL decided

## Known hardening work

- Zeroize stored passwords.
- Add a real archive corpus and fuzz targets.
- Add a page-count or total-memory budget.
- Add CI for macOS and Linux.
- Revisit 7z per-page reopen if large books are slow.
- Preserve RAR read-time I/O error kind if callers need it.
- Decide whether symlinked directory pages should be followed.
