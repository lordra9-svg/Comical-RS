# Code map

Every tracked source, test and project file, with the behavior it owns and the
tests that protect it. Use this as the starting point when tracing a failure.

## Root files

| File | Purpose |
|---|---|
| `Cargo.toml` | Workspace members, shared package metadata, shared dependencies |
| `Cargo.lock` | Exact dependency versions for reproducible builds |
| `.gitignore` | Ignores build output and local fixtures |
| `LICENSE` | MIT license for this repository |
| `README.md` | User-facing overview, usage and milestone status |
| `CLEANROOM.md` | Access rules for the clean-room implementation |
| `PROVENANCE.md` | Roles, attestations and access log |
| `THIRD_PARTY_LICENSES.md` | Dependency license inventory and election rules |
| `spec/behavior.md` | Behavior contract. Not implementation documentation |
| `docs/` | Implementation documentation. Start at `docs/README.md` |
| `tests/vectors/README.md` | How to read the JSON vectors |
| `tests/vectors/spreads.json` | Spread state cases |
| `tests/vectors/zoom.json` | Zoom and fit cases |
| `tests/vectors/ui.json` | Seed right-to-left screen mapping cases |
| `tests/vectors/images.json` | 42 oracle-confirmed filter cases and source bytes |

## Documentation

| File | Purpose |
|---|---|
| `docs/README.md` | Index, source-of-truth rules and quick commands |
| `docs/architecture.md` | Crate boundaries, data flow and design rules |
| `docs/code-map.md` | This file |
| `docs/core.md` | Spread state, zoom math, invariants and quirks |
| `docs/archive.md` | Archive trait, formats, errors, caps and passwords |
| `docs/image.md` | Decode, rotation, filters, golden hashes and image maintenance |
| `docs/testing.md` | Test layers, fixtures, vector schemas and debugging |
| `docs/runbook.md` | Build, diagnose, change and release procedures |
| `docs/roadmap.md` | Placeholders, planned milestones and known gaps |

## `comical-core`

Pure domain types. No dependencies, no I/O, no UI, no image code.

| File | Owns |
|---|---|
| `crates/comical-core/Cargo.toml` | No runtime dependencies; `serde` and `serde_json` are test-only |
| `src/lib.rs` | `PageEntry`; re-exports the public spread and zoom API |
| `src/spread.rs` | Page orientation, page parts, spread state and navigation |
| `src/zoom.rs` | Zoom modes, fit math, oversize tests and target dimensions |
| `tests/vectors.rs` | Loads `spreads.json` and `zoom.json` and asserts every case |
| `tests/spreads_generated.rs` | Generated spread invariants for N = 1 through 12; boundary regressions |
| `tests/zoom_generated.rs` | Generated zoom cases; square, tiny, huge, neighbor and oversize regressions |

### `src/spread.rs`

Key public symbols:

- `PageOrientation`: `Portrait` or `Landscape`, after rotation. Square is portrait.
- `PagePart`: `Full`, `Left`, `Right`.
- `PageRef`: `{ page, part }`.
- `Spread`: `{ left, right }`, in book order.
- `ViewMode`: `OnePage`, `TwoPage`.
- `SpreadError`: `EmptyBook`, `InvalidSpread`.
- `SpreadState`: page orientations, mode and current spread.

Key private helpers:

- `first_spread`, `last_spread`, `start_spread`.
- `page_ref` (landscape becomes left half, otherwise full).
- `previous_page_ref` (landscape becomes right half, otherwise full).
- `next_page`, `previous_page` (clamped).
- `advance_after`.

Invariants:

- `left` is always present and in range.
- `right` is absent only in one-page mode or for a one-page book.
- `left.page <= right.page`.
- A split landscape page appears as `left=Left, right=Right` on the same index.
- A right page may be a full portrait or the left half of a landscape page.
- A left page may be a full portrait or a half of a landscape page.
- Navigation clamps at both ends.

Tests: `tests/vectors.rs`, `tests/spreads_generated.rs`.

### `src/zoom.rs`

Key public symbols:

- `Size`, `ZoomMode`, `ZoomOptions`, `ZoomResult`.
- `MIN_CUSTOM_PERCENT = 1`, `MAX_CUSTOM_PERCENT = 200`.
- `compute_zoom`, `compute_zoom_with_neighbors`.
- `is_oversized`.

Key private helpers:

- `fit_scale`, `fit_width_base`, `fit_width_scale`.
- `fit_height_base`, `fit_height_scale`.
- `fit_usable_width`, `fit_width_scrollbar_width`.
- `oversize_width`, `oversize_height`.
- `custom_scale`, `result`, `scaled_dimension`.
- `is_wide` (`width >= height`), `is_portrait` (`height >= width`).

Tests: `tests/vectors.rs`, `tests/zoom_generated.rs`.

## `comical-archive`

Read-only access to books. Owns format dispatch, page filtering, sorting, size
caps, error mapping and the archive trait.

| File | Owns |
|---|---|
| `Cargo.toml` | Reader-only features for `zip`, `rars`, `sevenz-rust` |
| `src/lib.rs` | Public trait, options, errors, format dispatch, filtering and sorting |
| `src/zip.rs` | ZIP reader |
| `src/rar.rs` | RAR reader and multi-volume rejection |
| `src/sevenz.rs` | 7z reader and callback-order mapping |
| `src/directory.rs` | Recursive directory book |
| `tests/archive.rs` | Format, encryption, size-cap and error integration tests |

### `src/lib.rs`

Key public symbols:

- `DEFAULT_MAX_ENTRY_BYTES` (512 MiB).
- `ArchiveFormat`: `Zip`, `Rar`, `SevenZ`, `Directory`.
- `OpenOptions`: optional password and entry cap.
- `ComicArchive`: `format`, `pages`, `read_page`.
- `ArchiveError`.
- `open`, `open_with_options`, `format_for`.

Key private symbols:

- `is_image_name`.
- `Candidate<S>`.
- `select_pages` (stable byte sort; page list plus source list).
- `page_count_error`, `check_index`, `check_declared_size`.

Invariants:

- The public page order is the byte order of full names.
- Duplicate names are kept.
- A book always has at least one page.
- `read_page` checks index, then declared size, then the actual read.
- Format dispatch is by extension, except directories, which win by `is_dir()`.

Tests: unit tests in `lib.rs`, plus `tests/archive.rs` and CLI tests.

### `src/zip.rs`

- Opens a `File` and a `ZipArchive`.
- Enumerates entries with `by_index_data` for metadata.
- Uses `entry.name()` to decode UTF-8 or CP437 names.
- Stores archive indices as page sources.
- Reads with `by_index_with_options` and an optional password.
- Caps the read with `take(limit + 1)`.
- Maps ZIP errors to `ArchiveError`.

Failure points:

- `ZipError::InvalidPassword` becomes `BadPassword`.
- The password-required sentinel becomes `PasswordRequired`.
- Unknown compression becomes `Unsupported`.
- Open-time I/O becomes `Open`; read-time I/O becomes `Io`.
- Other parse failures become `Corrupt`.

Tests: `zip_store_filters_and_sorts_pages`, `zip_deflate_extracts_exact_bytes`,
`zip_aes256_password_is_supported_and_wrong_password_is_rejected`,
`zip_cp437_name_without_utf8_flag_is_decoded`, size-cap tests.

### `src/rar.rs`

- Opens with `rars` and an optional password.
- Rejects archives whose main header says volume, and entries split before or
  after, with `Unsupported("multi-volume RAR")`.
- Skips directories and redirections.
- Decodes names with `decoded_name(None)`, lossy UTF-8, then converts `\` to `/`.
- Stores member indices as page sources.
- Reads with `read_member_at_with_options` and `with_max_member_output_bytes`.
- Maps `rars` errors to `ArchiveError`.

Failure points:

- Password-required and bad-password errors can occur at open for encrypted
  headers, or at read for encrypted data.
- `Io` at read loses the original error kind and becomes `io::Error::other`.
- A member with no payload is reported as corrupt.

Tests: `rar_lists_and_extracts`, `rar_password_is_supported_and_wrong_password_is_rejected`,
`rar_versions_from_14_through_7_list_and_extract_exact_bytes`,
`solid_rar5_and_rar7_read_every_page`,
`encrypted_rar5_and_rar7_data_require_the_correct_password`,
`encrypted_rar5_and_rar7_headers_require_the_correct_password`,
`multi_volume_rar_is_rejected_instead_of_silently_incomplete`.

### `src/sevenz.rs`

- Opens `SevenZReader` to parse the archive and headers.
- Builds a callback-order map because `for_each_entries` order is not file order.
- Includes folder streams first, then files without a folder.
- Maps an image file index to its callback position and stores that position.
- Reads a page by reopening the archive, walking callbacks and counting until the
  target position.
- Stops the current folder with `Ok(false)` and skips later folders once found.
- Verifies the entry name at the target position.
- Caps the read with `take(limit + 1)`.

Failure points:

- Encrypted 7z headers fail at open.
- If the callback order changes between open and read, the name check reports a
  corrupt archive instead of returning the wrong page.
- Returning `Ok(false)` stops only the current folder; do not assume it stops the
  whole traversal.
- Reopening per page is simple but costs a parse per page. Optimize only with a
  test that proves the order and memory behavior.

Tests: `sevenz_lists_and_extracts`,
`sevenz_aes256_password_is_supported_and_wrong_password_is_rejected`,
size-cap tests.

### `src/directory.rs`

- Recursively walks a directory.
- Sorts entries by file name at each level for deterministic traversal.
- Skips anything that is not a directory or a regular file, including symlinks.
- Builds `/`-separated paths relative to the book root.
- Stores relative paths as page sources.
- Reads with a bounded `take(limit + 1)`.

Tests: `directory_is_walked_recursively_and_sorted`, CLI directory test,
size-cap test.

## `comical-cli`

| File | Owns |
|---|---|
| `Cargo.toml` | Binary definition, `clap`, test dependencies |
| `src/main.rs` | `probe` and `extract` commands, exit codes, error text |
| `tests/cli.rs` | End-to-end binary tests |

`probe` prints one line per page:

```text
index<TAB>size<TAB>name
```

`extract` writes exact bytes to the given path. It checks the index before
reading and does not create a file for an out-of-range request.

Tests cover all three formats, byte ordering, suffix filtering, directories,
AES ZIP passwords, out-of-range indexes, corrupt input and write failures.

## `comical-image`

Opaque RGB8 decode, quarter-turn rotation and six-filter resampling. It does not
know about archives, spreads or UI.

| File | Owns |
|---|---|
| `Cargo.toml` | `image` with PNG, JPEG and GIF only; `thiserror`; test hashes |
| `src/lib.rs` | `RgbImage`, buffer invariants and allocation helpers |
| `src/decode.rs` | Format allowlist, header pixel cap, decode to RGB8 |
| `src/error.rs` | `ImageError` variants and messages |
| `src/rotate.rs` | `Rotation` and index-remapped quarter turns |
| `src/resize.rs` | `Filter`, contributions, two passes and target validation |
| `tests/decode.rs` | PNG RGB/RGBA, JPEG, GIF first frame and bad input |
| `tests/images.rs` | All 42 golden filter hashes |
| `tests/transform.rs` | Rotations, zero targets, buffer and dimension checks |

Key public symbols:

- `RgbImage`: validated constructor, dimensions, pixels, `pixel`.
- `Rotation`: `None`, `Cw90`, `Cw180`, `Cw270`.
- `Filter`: `Box`, `Bilinear`, `Bicubic`, `BSpline`, `CatmullRom`, `Lanczos3`.
- `decode`, `rotate`, `resize`.
- `ImageError`.
- `MAX_DECODED_PIXELS`.

Invariants:

- `width > 0`, `height > 0`.
- `pixels.len() == width * height * 3`.
- Only RGB8 is exposed; alpha is dropped at decode.
- Resize target sides are at least 1.
- Decode and resize return errors instead of panicking on bad input.

Details: `docs/image.md`.

## `comical-app`

Desktop viewer. eframe/egui shell over the three product crates. Details:
`docs/app.md`.

| File | Owns |
|---|---|
| `Cargo.toml` | `eframe`, `egui`, `rfd`, `lru`, `thiserror`, product crates |
| `src/lib.rs` | Module wiring, `run()`, window defaults (600x400 at 50,50) |
| `src/main.rs` | `comical` binary, first CLI argument is the book path |
| `src/app.rs` | `App`, `OpenedBook`, natural and texture caches, opening, navigation, zoom and pan state, orientation learning |
| `src/cache.rs` | `ByteSize`, `ByteLru`: byte-budgeted LRU cache backing both page caches |
| `src/view.rs` | Canvas drawing, content geometry, pan clamping, placeholder text |
| `src/nav.rs` | Keyboard snapshot and map |
| `src/error.rs` | `AppError` |
| `tests/open_book.rs` | Directory book opens; page 0 decodes at natural size |

Key symbols:

- `App`, `AppError`, `run`.
- `OpenedBook`, `TextureKey`, `split_size`, `crop_part`.
- `ByteSize`, `ByteLru`.
- `handle_input`, `Pressed`.
- `draw`, `draw_book`, `visible_refs`, `page_sizes`, `content_bounds`,
  `clamp_pan`, `paint_placeholder`.

Invariants:

- `zoom.mode` always mirrors `SpreadState::mode`.
- Only one `AppError` exists, in `error.rs`.
- Modules are declared once, in `lib.rs`.
- `TextureKey` includes the target size, so a zoom change cannot reuse a stale
  texture.
- Panning hits the texture cache: no decode, resize or upload.
- Both caches are bounded by bytes (`cache_bytes`, `resample_bytes`), not by a
  fixed entry count; an oversized entry is admitted and evicts the rest.
- A page that fails to decode does not take down the book.

## `xtask`

Development-only fixture generator. Never shipped.

| File | Owns |
|---|---|
| `Cargo.toml` | Writer features for `zip`, `rars`, `sevenz-rust` |
| `src/lib.rs` | ZIP, RAR and 7z fixture writers; CP437 manual ZIP; sample set |
| `src/main.rs` | Generates a directory of fixtures for manual testing |

Key symbols:

- `FixtureEntry`, `ZipMethod`, `ZipEncryption`.
- `write_zip`, `write_cp437_zip`.
- `RarVersion`, `RarFixtureOptions`, `write_rar`, `write_rar_with_options`,
  `write_rar_volume_set`.
- `write_7z`, `write_7z_encrypted`.
- `write_corrupt`, `write_sample_set`, `SAMPLE_ENTRIES`.

The writers exist only in this crate and in dev-dependencies. Do not move them
into `comical-archive`.

## Where to fix common problems

| Symptom | First file | First test |
|---|---|---|
| Wrong page order | `comical-archive/src/lib.rs` (`select_pages`, `is_image_name`) | `zip_store_filters_and_sorts_pages` |
| Page missing from list | format reader and `is_image_name` | matching format test |
| Wrong bytes returned | format reader `read_page` | exact-byte tests |
| Password rejected | format error mapping | password tests |
| Size cap not enforced | reader `read_page` and `check_declared_size` | `every_reader_enforces_the_size_cap` |
| Spread skips a page or turn | `comical-core/src/spread.rs` | `spreads_generated.rs` and vector case |
| Zoom size off by one | `comical-core/src/zoom.rs` truncation and comparison | `zoom_generated.rs` |
| Scrollbar reserved when it should not be | fit base and neighbor checks | neighbor tests |
| Image filter hash mismatch | `comical-image/src/resize.rs` pass order, window, rounding | `tests/images.rs` |
| Image decode fails | `comical-image/src/decode.rs` allowlist and limits | `tests/decode.rs` |
| Rotation wrong | `comical-image/src/rotate.rs` index mapping | `tests/transform.rs` |
| CLI output format changed | `comical-cli/src/main.rs` | `tests/cli.rs` |
| Fixture writer wrong | `xtask/src/lib.rs` | integration test using it |
| Licensing question | `THIRD_PARTY_LICENSES.md`, `Cargo.lock` | `cargo metadata` review |
