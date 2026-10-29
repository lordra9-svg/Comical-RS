# Archive subsystem

The archive layer opens a book, produces a sorted page list and reads exact
page bytes. It is read-only.

Source: `crates/comical-archive`.

## Public API

```rust
pub trait ComicArchive: Send {
    fn format(&self) -> ArchiveFormat;
    fn pages(&self) -> &[PageEntry];
    fn read_page(&mut self, index: usize, limit: u64) -> Result<Vec<u8>, ArchiveError>;
}
```

```rust
pub struct OpenOptions<'a> {
    pub password: Option<&'a str>,
    pub max_entry_bytes: u64,
    pub natural_sort: bool,
}
```

Entry points:

- `open(path)`: default options, no password, 512 MiB entry cap.
- `open_with_options(path, options)`: explicit password, cap and sort order.
- `OpenOptions::with_natural_sort(bool)`: numeric-aware ordering, off by
  default.
- `format_for(path)`: extension to `ArchiveFormat`, without opening.
- `DEFAULT_MAX_ENTRY_BYTES`: 512 MiB.

## Open pipeline

1. If the path is a directory, use the directory reader. The extension is ignored.
2. Otherwise map the extension:
   - `.zip`, `.cbz` -> ZIP
   - `.rar`, `.cbr` -> RAR
   - `.7z`, `.cb7` -> 7z
3. Unknown extension -> `ArchiveError::CannotOpen`.
4. The reader enumerates entries and creates a `Candidate` for each image:
   `{ name, size, source }`.
5. `select_pages` stable-sorts candidates by the byte value of `name`, or by
   natural order when `OpenOptions::natural_sort` is on.
6. It splits the result into the public `Vec<PageEntry>` and a private
   `Vec<Source>`.
7. Empty page list -> `ArchiveError::NoPages`.
8. The reader is boxed as `ComicArchive`.

The default sort is not natural sort and not locale-aware: `page10` sorts
before `page2`. The sort is stable, so duplicate names keep archive order.

With `natural_sort` on, runs of ASCII digits compare by numeric value, so
`page2` sorts before `page10`. Everything outside a digit run still compares by
byte value, which makes natural order a strict refinement of the default: only
names that differ inside a number can move. Equal values order by run length
(`2` before `02`), so the comparator is total. The viewer exposes this as the
`natural_sort` setting; the CLI exposes it as `--natural-sort`.

## Page filter

Accepted suffixes, case-insensitive:

```text
.jpg  .jpeg  .gif  .png
```

The suffix must be at the end of the full name. `fake.png.txt` is not a page.
Directories are skipped. The filter does not inspect file contents. A `.png`
entry containing non-image bytes is still a page and fails later in the image
pipeline.

## Declared size and caps

Each page carries the archive's declared uncompressed size. `read_page` does:

1. Bounds check.
2. `check_declared_size(name, page.size, limit)` before allocation.
3. Reader-specific read with a hard cap of `limit + 1`.
4. If the observed length exceeds `limit`, return `EntryTooLarge`.

This double check protects against incorrect archive metadata and decompression
bombs. `limit + 1` uses saturating addition. A limit of `u64::MAX` therefore
reads without a practical cap, so callers should pass a sane limit.

The CLI always passes `DEFAULT_MAX_ENTRY_BYTES`.

## Password flow

- `OpenOptions.password` is an optional borrowed string.
- Each book stores `Option<String>` for its lifetime.
- ZIP and RAR can list entries without a password when only data is encrypted.
- RAR with encrypted headers fails at open with `PasswordRequired`.
- 7z with encrypted headers fails at open with `PasswordRequired`.
- Wrong passwords map to `BadPassword`.
- No password is written to disk or included in `probe` output.

Hardening gap: the stored password is not zeroized. A future `secrecy` wrapper
would reduce exposure. Do not add it without tests that preserve the public API.

## Format matrix

| Feature | ZIP | RAR | 7z | Directory |
|---|---:|---:|---:|---:|
| List pages | Yes | Yes | Yes | Yes |
| Read pages | Yes | Yes | Yes | Yes |
| Store / no compression | Yes | Yes | Yes | n/a |
| Deflate | Yes | n/a | n/a | n/a |
| LZMA / LZMA2 | n/a | n/a | Yes | n/a |
| BCJ / Delta | n/a | n/a | Crate-dependent; not tested | n/a |
| Solid archives | n/a | Yes | Crate support; not tested | n/a |
| Encrypted data | AES-256 | Yes | AES-256 | n/a |
| Encrypted headers | n/a | Yes | Fixture requires password at open | n/a |
| CP437 names | Yes | Legacy names decoded by crate | n/a | n/a |
| Multi-volume | n/a | Rejected | Not supported | n/a |
| ZIP64 | Crate support | n/a | n/a | n/a |
| ZipCrypto | Crate support | n/a | n/a | n/a |

Synthetic tests cover the tested features. Rows marked "Crate support" are
available through the dependency but are not proven by this repository's tests.
`tests/corpus/` adds Info-ZIP and 7-Zip interoperability coverage (including
solid 7z and ZipCrypto). RAR samples from an external RAR writer are still
missing.

## ZIP reader

File: `src/zip.rs`.

Dependencies and features:

```toml
zip = { version = "9.0.5", default-features = false,
        features = ["deflate-flate2-zlib-rs", "aes-crypto"] }
```

Open:

- `File::open`.
- `ZipArchive::new`.
- Enumerate with `by_index_data(index)` for metadata.
- `entry.name()` returns a decoded name. The crate handles the UTF-8 flag and
  CP437 fallback.
- Skip directories and non-images.
- Keep the archive index as the page source.

Read:

- `by_index_with_options(index, ZipReadOptions::new().password(...))`.
- `take(limit + 1)` into a `Vec`.
- Observed overflow -> `EntryTooLarge`.

Error mapping:

| `zip` error | `ArchiveError` |
|---|---|
| `InvalidPassword` | `BadPassword` |
| `UnsupportedArchive(PASSWORD_REQUIRED)` | `PasswordRequired` |
| Other `UnsupportedArchive` | `Unsupported` |
| `CompressionMethodNotSupported` | `Unsupported` |
| Open-time `Io` | `Open { path, source }` |
| Read-time `Io` | `Io` |
| Other | `Corrupt` |

Note: `map_zip_open_error` only turns `Io` into `Open`; all other open errors go
through the same mapping as read errors.

## RAR reader

File: `src/rar.rs`.

Dependency and features:

```toml
rars = { version = "0.11", default-features = false, features = ["encryption"] }
```

Open:

- `ArchiveReader::read_path_with_options` with optional password bytes.
- Reject if the main header says volume.
- Enumerate members; reject if any member is split before or after.
- Skip directories and redirections.
- `decoded_name(None)` returns bytes; the reader uses lossy UTF-8 and converts
  `\` to `/`.
- Keep the member index as the page source.

Read:

- `read_member_at_with_options(index, options)`.
- `with_max_member_output_bytes(limit)` enforces the cap in the reader.
- Missing payload -> `Corrupt`.

Multi-volume policy:

- The archive-level `archive_is_volume` check catches volume sets.
- Entry-level `is_split_before` / `is_split_after` catches split members.
- Both return `Unsupported("multi-volume RAR")`.
- The code never returns a partial page list.

Error mapping:

| `rars` error kind | `ArchiveError` |
|---|---|
| `PasswordRequired` | `PasswordRequired` |
| `BadPassword` | `BadPassword` |
| `UnsupportedFeature` | `Unsupported` |
| Open-time `Io` | `Open { path, source }` |
| Read-time `Io` | `Io(io::Error::other(...))` |
| Other | `Corrupt` |

Tradeoff: read-time I/O loses the original error kind because the code rebuilds
an `io::Error` from the message. If error-kind precision matters, change
`map_read_error` and add a test.

## 7z reader

File: `src/sevenz.rs`.

Dependency and features:

```toml
sevenz-rust = { version = "0.6.1", default-features = false,
                features = ["aes256", "compress"] }
```

`compress` is required even though this is read-only: the `aes256` feature in
0.6.1 does not compile without the encoder module. Do not remove it without
checking the crate.

Open:

- `SevenZReader::open(path, Password::from(password))`.
- Parse the archive header. Encrypted headers fail here.
- `callback_order` builds the order in which `for_each_entries` will visit files:
  1. For each folder, walk its file-index range and include files with a stream
     and a non-zero size.
  2. Then append every file not assigned to a folder.
- Build an ordinal map from file index to callback position.
- Keep the callback position as the page source, not the file index.

Read:

- Reopen the archive with the password.
- Walk `for_each_entries`.
- Count every yielded entry until `seen == target`.
- Verify the entry name equals the page name.
- Read with `take(limit + 1)`.
- Return `Ok(false)` after reading or on overflow, then skip all later folders.

Why reopen: the reader consumes folders as a stream. Keeping decoded state for
all pages would increase memory and couple page reads to traversal order.
Reopening is simple and correct, but costs one header parse per page.

Why `Ok(false)` is not enough to stop: it stops the current folder, not the
whole archive. The callback checks `found` and `overflow` first and returns
`Ok(false)` for later folders until traversal ends.

Error mapping:

| `sevenz_rust` error | `ArchiveError` |
|---|---|
| `PasswordRequired` | `PasswordRequired` |
| `MaybeBadPassword` | `BadPassword` |
| `Unsupported` | `Unsupported` |
| `UnsupportedCompressionMethod` | `Unsupported` |
| Open-time `Io` | `Open { path, source }` |
| Read-time `Io` | `Io`, with context when present |
| Other | `Corrupt` |

The name check at the target position is intentional. If a future crate upgrade
changes callback order, the reader reports a corrupt archive instead of
returning the wrong page.

## Directory reader

File: `src/directory.rs`.

- `fs::read_dir` recursively.
- Sort each directory's entries by file name before descending.
- Only directories and regular files. Symlinks are skipped.
- Names are relative to the book root and joined with `/`.
- Read with `File::open` and a bounded `take(limit + 1)`.

The final page sort is independent of traversal order. Traversal sorting only
makes duplicate handling and fixture generation deterministic.

## Adding a new archive format

1. Add a variant to `ArchiveFormat`.
2. Add the extension mapping in `format_for`.
3. Add a module with `open(path, options) -> Result<Box<dyn ComicArchive>, ArchiveError>`.
4. Enumerate image entries into `Candidate` and call `select_pages`.
5. Return `NoPages` for an empty result.
6. Implement `read_page` with index check, declared-size check, bounded read and
   observed-size check.
7. Map reader errors to `ArchiveError`.
8. Add the reader dependency with the smallest feature set.
9. Update `THIRD_PARTY_LICENSES.md`, `docs/archive.md` and `docs/code-map.md`.
10. Add tests for list, exact bytes, sort, filter, cap, corruption, password and
    any format-specific unsupported features.

Do not add a format by extracting to a temporary directory. Keep the read in
memory or in the reader.

## Testing the archive layer

Integration tests: `crates/comical-archive/tests/archive.rs`.

They use `xtask` writers, so they do not depend on external archive files.
Coverage includes:

- ZIP store, deflate, AES-256, CP437 names, ordering.
- RAR 1.4, 1.5, 2.0, 2.9, 3.0, 4.0, 5.0 and 7.0.
- Solid RAR5 and RAR7.
- Encrypted RAR5 and RAR7 data and headers.
- Multi-volume RAR rejection.
- 7z LZMA2 and AES-256.
- Directory recursion and slash paths.
- Out-of-range index, size cap, corrupt input, no pages, unsupported extension.

The tests prove code paths against synthetic fixtures. They do not prove
interoperability with every real-world writer. A real corpus is still required.

## Known archive limitations

- Multi-volume RAR is rejected, not read.
- Multi-volume 7z is not supported.
- No archive writing.
- No recovery or repair.
- Natural sort is opt-in; the spec default stays byte order.
- No page count limit.
- No zeroization of stored passwords.
- 7z reads reopen the file per page.
- RAR read errors lose the original I/O kind.
- Directory symlinks are skipped.
- RAR writer corpus and fuzzing are still open.
- Solid 7z from real 7-Zip requires prior entries to be fully drained while
  seeking; `src/sevenz.rs` does that (sevenz-rust issue #31).
