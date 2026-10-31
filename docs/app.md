# `comical-app`: desktop viewer

`comical-app` is the eframe/egui desktop shell. It opens a book, drives
`comical-core` for spreads and zoom, asks `comical-image` for pixels, and paints
the result on a black canvas. It reimplements no spread or zoom rule.

This is the M6 slice: one window, black canvas, both view modes, five zoom
modes, keyboard navigation, drag pan, fullscreen, CLI path, file dialog,
toolbar, thumbnail browser, per-page rotation, RTL, and persistent settings.

## Files

| File | Owns |
|---|---|
| `src/lib.rs` | Module wiring, `run()`, window and viewport defaults |
| `src/main.rs` | `comical` binary: first CLI argument is the book path |
| `src/app.rs` | `App`, `OpenedBook`, caches, book opening, navigation, pan, zoom state |
| `src/view.rs` | Canvas drawing, content geometry, pan clamping, placeholder text |
| `src/nav.rs` | Keyboard map |
| `src/error.rs` | `AppError` |
| `tests/open_book.rs` | Opens real directory books: natural size, sort order, remembered page, filter |

## State

```text
App
  book: Option<OpenedBook>
  spread: Option<SpreadState>
  orientations: Vec<PageOrientation>   learned lazily from decoded pages
  changed_pages: Vec<usize>            orientations learned since last rebuild
  zoom: ZoomOptions                    mode always mirrors the spread state
  pan: (f32, f32)                      canvas points
  fullscreen: bool
  error: Option<String>                book-level error, drawn in the canvas
  title, title_dirty
  settings: Settings                   persisted TOML, see below
  book_key: String                     canonical path, used by remember_page
  pending_password_path: Option<String>
                                        open attempt waiting for a password
  password_input: String              persistent prompt text across frames

OpenedBook
  pages: Vec<PageEntry>
  archive: Box<dyn ComicArchive>
  page_sizes: Vec<Option<Size>>        natural size, filled on first decode
  page_errors: Vec<Option<String>>     per-page failure, not retried
  natural: LruCache<usize, RgbImage>   decoded full pages, capacity 10
  textures: LruCache<TextureKey, TextureHandle>
                                       uploaded, resized, capacity 10
  filter: Filter                       resampling filter for scaled pages
```

`TextureKey` is `(page, part, rotation, filter)` and is `Copy`. A hit costs no
archive read, no decode, no resample and no upload, which is what keeps drag
panning inside the frame budget. Because the filter is part of the key,
changing it never shows pixels from the previous filter.

Opening a book replaces `OpenedBook` only after the archive opens, so a failed
open leaves the previous book on screen with the error drawn over it.

## Frame sequence

1. `title_dirty` sends `ViewportCommand::Title`.
2. `nav::handle_input` reads one keyboard snapshot and applies at most one
   navigation, one view-mode and one zoom change.
3. `view::draw` paints the frame.
4. If the spread changed during the frame, request a repaint.

### Orientation learning

Page orientation is not known from the archive. `view::draw` runs two passes:

1. Resolve the visible page refs, call `App::page_size` for each, then call
   `App::refresh_spread`. If a newly learned orientation changed the spread,
   re-read the spread and fill sizes for any page it added.
2. Compute zoom, fetch textures and paint.

Learning before painting avoids showing a landscape page as a full portrait
page for one frame. A landscape page in the left slot restarts the spread as
its two halves; a landscape page in the right slot has its part changed to
`PagePart::Left`, which keeps the left page visible.

## Zoom

`ZoomOptions::new(ViewMode::TwoPage, ZoomMode::Width)`, matching the behavior
defaults. `custom_percent` starts at 100 and is clamped to 1 through 200.

Every visible page is scaled as a whole page through
`compute_zoom_with_neighbors`. The other visible page is passed as the neighbor,
which is what makes the two-page scrollbar decisions agree. A split landscape
page is then divided at the midpoint of the scaled dimensions:
`split_size` gives the leftover pixel column of an odd width to the right half.

`scrollbar` is 0 and `fit_only_if_oversized` is false. The application does not
draw scrollbars; it pans instead. See the divergences below.

## Rendering geometry

- Canvas is the full `CentralPanel`, physical size is `points * pixels_per_point`.
- Two-page spreads meet at the canvas center: the left page's right edge and the
  right page's left edge both sit at `canvas_width / 2`.
- A single page is centered horizontally.
- Content is centered vertically when it fits, and top-aligned when it does not.
- Pan is clamped so the content never leaves a gap at a canvas edge.
- `RgbImage` becomes `ColorImage::from_rgb` and is uploaded once per
  `TextureKey` with `TextureOptions::LINEAR`.
- A page that fails to decode paints a dark placeholder with the wrapped error,
  clipped to that page's rect. Other pages in the spread still render.

## Input

| Key | Action |
|---|---|
| Right, Page Down, J | Next turn |
| Left, Page Up, K | Previous turn |
| Down, Space | Next page |
| Up, Backspace | Previous page |
| Home, End | First, last spread |
| 1, 2 | One-page, two-page |
| F | Cycle Fit, Width, Height, Full, Custom |
| W, H, O, C | Fit width, fit height, original, custom |
| `+`, `=`, `-` | Custom percent by 10 |
| 0 | Custom 100% |
| F11 | Fullscreen |

Wheel scroll pans the content; Ctrl+wheel steps the custom zoom by 10 points.
Left drag pans. Pan resets on every page, view-mode and zoom change.

## Settings that change behavior

`src/settings.rs` persists a versioned TOML file in the platform config
directory. These keys are read at startup and written on exit:

| Key | Effect |
|---|---|
| `zoom`, `zoom_percent`, `fit_only_oversize`, `mode`, `direction` | Initial view state |
| `toolbar_visible`, `browser_visible` | Panel visibility |
| `filter` | Resampling filter for scaled pages; picked in the settings window |
| `natural_sort` | Numeric-aware page order, applied on the next open |
| `remember_page` | Reopen a book on the page that was last shown |
| `last_directory` | Initial directory for the open dialog |
| `last_pages` | Per-book page map backing `remember_page`, capped at 200 books |
| `window_width`, `window_height`, `window_x`, `window_y` | Initial window geometry |

`last_pages` is an implementation key, not part of the behavior spec table: the
spec names the `remember_page` switch but not its storage. It is bounded so the
file cannot grow without limit, and books are keyed by canonical path so a
relative path, an absolute path and a symlink share one entry.

The settings window (`S`) exposes direction, layout, zoom, fit-only, filter,
natural sort, remember page and panel visibility. Changing the filter clears the
texture cache so the next frame decodes with the new kernel.

`cache_bytes` and `resample_bytes` drive two byte-budgeted LRU caches in
`src/cache.rs`. `cache_bytes` bounds decoded pages at natural size;
`resample_bytes` bounds uploaded page textures. An entry is measured at insert
time (`width × height × 3` for RGB pixels, `width × height × 4` for textures)
and the least-recently-used entries are evicted until the new entry fits. An
entry larger than its budget is still admitted and evicts everything else, so
a single huge page cannot deadlock the viewer. Keeping textures in a separate
budget means a book with many zoom levels cannot push decoded pages out.

## Password prompt

Encrypted books list without a password and fail at read time, so the viewer
detects the missing password through the decode result (`ArchiveError::
is_password_required`) and opens the prompt window instead of a placeholder.

The window asks for the password, submits it with `App::submit_password`, and
retries the open. A wrong password keeps the prompt open; the error is drawn in
the canvas. `App::cancel_password_prompt` closes it and keeps the previous
book. Passwords are never persisted.

## Toolbar (`toolbar.rs`)

A top panel drawn by `egui::TopBottomPanel::top` with ten buttons in spec order:
rotate left CCW, rotate left CW, first, previous turn, previous page, next
page, next turn, last, rotate right CCW, rotate right CW. All buttons are
disabled until a book is open. Visibility defaults to on and is toggled with
the `T` key; the state syncs to `Settings::toolbar_visible`. The toolbar is
hidden in fullscreen mode. Button clicks produce a `ToolbarAction` enum that
`apply()` dispatches to the corresponding `App` method, keeping the immediate-
mode draw pass free of mutable borrows.

## Thumbnail browser (`browser.rs`)

A left side panel 110 px wide with a white background. Each row is 60 px tall
with 5 px horizontal and 3 px vertical margins. Thumbnails are generated on
demand with `comical_image::resize` using the Box filter and cached in an LRU
cache (`lru::LruCache<usize, TextureHandle>`) keyed by page index. The cache
lives on `App` and is cleared when a new book is opened. Clicking a row calls
`SpreadState::go_to_page`. Selection follows the current page. Visibility
defaults to on and is toggled with the `B` key; the state syncs to
`Settings::browser_visible`. The browser is hidden in fullscreen mode.

## Divergences from `spec/behavior.md`

These are known and intentional for M4. Record them in the port plan before
changing either side.

| Spec | Current app | Reason |
|---|---|---|
| Arrow keys scroll 10 px | Arrows turn pages and step pages; pan is drag-only | M4 has no scrollbar or key-scroll path |
| Scrollbar thickness reserved when content overflows | `scrollbar = 0`; content pans with no reserved thickness | No scrollbars are drawn yet |

| Alt+Enter, Alt+O, Alt+X, F1 | F11 only | M4 scope |
| Context menu | Not implemented | Later milestone |

## Tests

- `app.rs` unit tests: defaults, navigation and pan reset, view-mode and zoom
  sync, percent clamping, orientation rebuild, split size, half cropping.
- `view.rs` unit tests: two-page centering, pan clamping.
- `tests/open_book.rs`: opens real directory books and checks natural size,
  byte order versus `natural_sort`, `remember_page` restore, and filter storage.

Not covered by automated tests: window creation, keyboard and pointer events,
fullscreen, and the native file dialog. Those are on the manual checklist.
