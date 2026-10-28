# Architecture

## Purpose

A behavior-compatible comic viewer for Windows, macOS and Linux. It reads image
entries from ZIP, RAR, 7z and directory books. The implementation is MIT. The
archive readers are permissive third-party crates.

The project is built in slices. The archive, core, image and M4 viewer slices
are implemented. Toolbar, thumbnails, rotation, right-to-left drawing and
settings are not.

## Workspace

```text
comical-cli
    |
    v
comical-archive  --->  comical-core
    |
    +--> zip, rars, sevenz-rust

comical-image    --->  image (PNG, JPEG, GIF decode)

comical-app  --->  comical-core, comical-archive, comical-image
    |
    +--> eframe, egui, rfd, lru, thiserror

xtask            (development only: writes synthetic test archives)
    +--> zip, rars, sevenz-rust writer features
```

## Dependency rules

| Crate | May depend on | Must not depend on |
|---|---|---|
| `comical-core` | Nothing | UI, image, archive, I/O, OS APIs |
| `comical-archive` | `comical-core`, archive readers, `thiserror` | UI, image, original source |
| `comical-cli` | `comical-archive`, `clap` | UI, image |
| `comical-image` | Image codecs, core types | Archive readers, UI |
| `comical-app` | All product crates | Test-only fixture writers |
| `xtask` | Archive writers | Product runtime code |

The important rule is that `comical-core` stays pure. It has no dependencies.
Spread and zoom logic can be tested without files, images or a window.

`xtask` is not shipped. It is a test and development dependency. Its writer
features never enter the product crates.

## Data flow

### Opening a book

```text
path
  -> directory?             -> recursive walk
  -> extension              -> ZIP / RAR / 7z reader
  -> reader enumerates entries
  -> suffix filter          -> only .jpg, .jpeg, .gif, .png
  -> candidate list         -> name, declared size, reader-specific source
  -> byte sort by full name
  -> page list + source list
  -> ComicArchive
```

The page list is public. The source list is private to the reader. Page identity
is the book-order index. Duplicate names are allowed and remain distinct pages.

### Reading a page

```text
index
  -> bounds check
  -> declared-size check
  -> reader-specific extraction
  -> bounded read
  -> exact bytes
```

No reader writes to the archive or extracts a page to disk. The CLI may write an
extracted page to a path the user names.

### Layout and zoom

```text
PageEntry list
  -> page orientations after rotation
  -> SpreadState
  -> current Spread (book order)
  -> screen mapping (future UI, direction-dependent)
  -> compute_zoom (canvas, page, options, neighbors)
  -> target size
  -> decode and resample (comical-image)
  -> draw (future UI)
```

`SpreadState` works in book order. Right-to-left reading does not change the
spread; it only changes which page is drawn on the screen left. That screen
mapping belongs to the UI layer, not the core.

### Image pipeline

```text
page bytes
  -> format allowlist          PNG / JPEG / GIF
  -> header pixel cap          MAX_DECODED_PIXELS
  -> decode to RGB8            alpha dropped
  -> rotate                    quarter turns
  -> resize                    six filters, two passes
  -> RgbImage
```

`comical-image` owns decode, rotation and resampling. It does not know about
archives, spreads, zoom modes or UI. Rotation happens before the UI asks the
core for page orientation. See `image.md`.

## Book model

A book is:

- An ordered list of `PageEntry { name, size }`.
- One reader-specific source handle per page.
- A format tag.
- An optional password held for the lifetime of the open book.

The declared `size` is used for early rejection. The actual read is still capped,
because archive metadata can be wrong.

## Error boundary

`comical-archive` returns `ArchiveError`. It has user-facing messages for:

- unsupported extension
- open or parse failure
- no pages
- index out of range
- entry above the size cap
- corrupt archive
- password required
- wrong password
- unsupported feature
- I/O failure

`comical-core` returns only `SpreadError` when a state cannot be constructed.
Navigation clamps instead of returning errors.

The CLI prints errors to stderr and exits non-zero. Library callers receive the
typed error.

## Security and resource boundaries

| Boundary | Rule |
|---|---|
| Entry size | Declared size checked before allocation; read capped with `limit + 1` |
| Password | Accepted as an option, held in memory, never written to disk |
| Archive writes | None |
| Extraction to disk | Only the CLI `extract` command, to a user-supplied path |
| Multi-volume RAR | Rejected, never partially listed |
| Corrupt input | Typed error, never a panic on normal malformed input |
| Page count | No explicit count cap; memory is bounded by entry metadata and book size |
| Decoded pixels | 100,000,000 pixel cap from header before decode |
| Resize target | Fallible RGB allocation; zero sides rejected |
| Image input | PNG, JPEG and GIF only; malformed input returns a typed error |

The password is a `String` while a book is open. It is not zeroized. This is a
known hardening gap, not a correctness bug.

## Threading

`ComicArchive: Send`, so a book can move to a worker thread. The trait uses
`&mut self` for reads, so one book is not shared for concurrent reads without a
lock. The current CLI is single-threaded.

## Design decisions

| Decision | Reason |
|---|---|
| Read-only archives | Viewer does not modify books |
| Byte-order sort by full name | Matches the behavior contract and is locale-independent |
| Stable sort | Duplicate names keep archive order |
| No natural sort | Kept out of v1; can be added as an option |
| Four suffixes only | `.jpg`, `.jpeg`, `.gif`, `.png`, case-insensitive |
| Directory books use `/` in names | Platform-independent book names |
| Multi-volume RAR rejected | Reading only the first part would silently lose pages |
| 7z reopened per page | The reader consumes a stream; reopening avoids holding decoded state |
| 32-bit float scale | Matches the behavior contract |
| `comical-core` has no dependencies | Pure, fast tests and no accidental coupling |

## Current limitations

- `comical-app` has no toolbar, thumbnails, rotation, RTL drawing, settings or
  password prompt. Arrow keys turn pages instead of scrolling.
- The app does not draw scrollbars; oversized content is panned with the mouse.
- No macOS or Linux CI has run. The viewer has only been run on Windows.
- Real-writer archive corpus covers Info-ZIP and 7-Zip; RAR writer samples and an image corpus are still open.
- No fuzzing or malformed-input campaign has run.
- No natural sort, WebP or AVIF. GIF uses the first frame only.
- No EXIF orientation handling.

See `roadmap.md` for the planned slices.
