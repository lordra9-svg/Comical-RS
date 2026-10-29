# Clean-room behavior specification: Comical-compatible comic viewer

Version: spec-v1 (draft)
Audience: the clean-room implementer. This document describes behavior only. Do not consult the original Comical source while implementing.

## 1. Scope

A desktop comic viewer for Windows, macOS and Linux. It opens ZIP, RAR and 7z archives of images and displays them one page or two pages at a time, with fit and zoom modes, rotation, right-to-left reading, thumbnails, fullscreen and keyboard and mouse navigation.

The viewer is read-only. It never modifies an archive and never extracts pages to disk.

## 2. Terms

- Page: one image entry in the archive or directory.
- Book order: the sorted order of page entries, index 0 to N-1.
- Landscape page: after applying rotation, width is greater than height.
- Portrait page: after applying rotation, height is greater than or equal to width.
- Wide for scaling: width is greater than or equal to height. A square page is wide for scaling, but it is portrait for spread pairing.
- Half: the left or right half of a landscape image.
- Spread: what is visible in two-page mode, made of a left page and a right page in book order.
- LTR: left-to-right reading. RTL: right-to-left reading.
- Canvas: the drawing area, black, excluding menus and side panels.
- Viewport: canvas size in physical pixels.
- Scrollbar thickness: the thickness reserved for a scrollbar when one is needed.

## 3. Inputs and page list

### 3.1 File types

- `.cbz` and `.zip` are ZIP archives.
- `.cbr` and `.rar` are RAR archives.
- `.cb7` and `.7z` are 7z archives.
- A directory is walked recursively.
- Extension matching is case-insensitive.

### 3.2 Page entries

Inside an archive or directory, include regular files whose names end, case-insensitively, in:

- `.jpg`
- `.jpeg`
- `.gif`
- `.png`

Ignore directories and other files. Include entries in subdirectories; use the full entry path.

### 3.3 Order

Sort the included full names by byte value, ascending. This is not a natural or numeric sort. For example, `page10.png` sorts before `page2.png`.

When the `natural_sort` setting (section 10) is enabled, compare runs of ASCII digits by numeric value instead: `page2.png` sorts before `page10.png`. Runs of equal value order by run length, shorter first (`2.png` before `02.png`). Text outside a digit run still compares by byte value, so natural order only moves names that differ inside a number.

An empty page list is an error: "This archive does not contain any pages."

### 3.4 Duplicate names

Keep duplicates in sorted order. Page identity is the entry index, not the name.

## 4. Page display model

### 4.1 Single-page mode

One page fills the canvas according to the zoom mode. The current page is that page.

### 4.2 Two-page mode

A spread contains:

- A left page reference: page index and part (full, left half, right half).
- A right page reference, or nothing at the end of the book.

The current page is always the book-order index of the left page. A page may appear in two consecutive spreads when it is landscape and split.

The viewer never mirrors an image. In RTL, the spread's pages swap screen sides, but each page keeps its own left and right halves as authored.

### 4.3 Constructing a spread from a starting page

For a starting page `p` in two-page mode:

- If `p` is landscape: show both halves of `p`, left half on the left and right half on the right.
- If `p` is portrait: show `p` full on the left. If `p+1` exists, show `p+1` on the right, using its left half if it is landscape, otherwise full. If `p+1` does not exist, the right side is empty.

### 4.4 First page

- Single-page mode, or N = 1: show page 0 alone.
- Page 0 is landscape: both halves of page 0.
- Otherwise: page 0 full on the left, page 1 on the right if it exists, using its left half if landscape, otherwise full.

### 4.5 Last page

- Single-page mode, or N = 1: show page N-1 alone.
- Page N-1 is landscape: both halves of page N-1.
- Otherwise: page N-1 full on the right. The left page is N-2, using its right half if landscape, otherwise full.

This gives book-cover behavior: the first page sits on the left, the last page on the right.

### 4.6 Next turn

A turn advances by a spread.

- If the right page is absent, show the last spread.
- If the right page is the last page, or the penultimate page and not its left half, show the last spread.
- If the right page is the left half of a landscape page: keep that page, move it to the left as its right half, and put the next page on the right, using its left half if landscape, otherwise full.
- Otherwise: construct a new spread starting at the page after the current right page.

### 4.7 Previous turn

A turn retreats by a spread.

- If the left page is page 0, or page 1 and not its right half, show the first spread.
- If the left page is the right half of a landscape page: keep that page, move it to the right as its left half, and put the previous page on the left, using its right half if landscape, otherwise full.
- If the result would fall before page 0, show the first spread.
- Otherwise: let `q` be the page before the current left page.
  - If `q` is landscape: show both halves of `q`.
  - If `q` is portrait: show `q` on the right, full. Show `q-1` on the left, using its right half if landscape, otherwise full. If `q-1` does not exist, show the first spread.

The turn operations are not always algebraic inverses at the boundaries. Use the expected vectors, not symmetry assumptions.

### 4.8 Next page and previous page

These move by one page rather than one spread.

- Next page:
  - If the left page is the last page, do nothing.
  - If the left page is the penultimate page and not its left half, show the last spread.
  - If the right page is the left half of a landscape page: show both halves of that same page in the spread.
  - Otherwise: shift the window one page forward. The old right page becomes the new left page, full or right half as it was. The next page becomes the new right page, using its left half if landscape, otherwise full.
- Previous page:
  - If the left page is page 0, do nothing.
  - If the left page is the right half of a landscape page: show both halves of that same page in the spread.
  - Otherwise: shift the window one page backward. The old left page becomes the new right page, using its left half if landscape, otherwise full. The previous page becomes the new left page, using its right half if landscape, otherwise full.

At the boundaries, clamp to the first or last spread.

### 4.9 Go to page

For a requested page `p`:

- Clamp `p` to 0..N-1.
- `p = 0`: first spread.
- `p = N-1`: last spread.
- Otherwise: construct a spread starting at `p`.

In single-page mode, show `p` alone.

### 4.10 Mode switch

- Single to double: derive the spread from the current page, using the first and last rules when the current page is an edge page.
- Double to single: keep the current page and center it.

### 4.11 Examples

N = 4, all portrait:

| Operation | Left | Right |
|---|---|---|
| First | 0 full | 1 full |
| Next turn | 2 full | 3 full |
| Next turn | 2 full | 3 full |
| Previous turn | 0 full | 1 full |
| Next page from first | 1 full | 2 full |

N = 4, page 1 landscape:

| Operation | Left | Right |
|---|---|---|
| First | 0 full | 1 left half |
| Next turn | 1 right half | 2 full |
| Next turn | 2 full | 3 full |
| Previous turn from [1 right, 2] | 0 full | 1 left half |
| Next page from first | 1 left half | 1 right half |
| Next page again | 1 right half | 2 full |

N = 4, page 2 landscape:

| Operation | Left | Right |
|---|---|---|
| First | 0 full | 1 full |
| Next turn | 2 left half | 2 right half |
| Next turn | 2 right half | 3 full |
| Previous turn from [2 right, 3] | 1 full | 2 left half |

## 5. Zoom and fit

All geometry is in physical pixels. `canvas_w` and `canvas_h` are the viewport size. `sb` is the scrollbar thickness.

Oriented page size:

- For north or south rotation: `w = image width`, `h = image height`.
- For east or west rotation: `w = image height`, `h = image width`.

Scale is a floating-point factor. Use 32-bit floats to match 0.8. Output dimensions truncate: `int(w * scale)`, `int(h * scale)`, at least 1.

### 5.1 Fit

If fit-only-if-oversized is enabled and the page is not oversized, scale = 1. Otherwise:

- If the page is wide for scaling or single-page mode: usable width = `canvas_w`.
- Otherwise: usable width = `canvas_w / 2`, integer division.
- If `canvas_w / canvas_h` (floating point) is greater than `w / h` (floating point): scale = `canvas_h / h`.
- Otherwise: scale = usable width / `w`.

### 5.2 Fit width

- Usable width = `canvas_w` when the page is wide for scaling or single-page mode, otherwise `canvas_w / 2`, integer division.
- Start with `scale = usable width / w`.
- If `scale * h` is not greater than `canvas_h`: keep it. No scrollbars.
- Otherwise:
  - Set `scale = canvas_h / h`.
  - Let `scrollbar_width = canvas_w - sb`, halved with integer division when the page is not wide for scaling and the mode is two-page. A square page uses the full scrollbar width.
  - If `int(scale * w)` is greater than `scrollbar_width`: keep this height-fit scale. It is the larger of the two, so the page fits fully.
  - Otherwise: set `scale = scrollbar_width / w`. The page fills the width and needs a vertical scrollbar.

In two-page mode, if this page uses the height-fit scale, check both neighbors. If either neighbor would use the scrollbar-present scale, use the scrollbar-present scale for this page.

### 5.3 Fit height

Symmetric to fit width:

- Start with `scale = canvas_h / h`.
- Let `usable width = canvas_w` when the page is wide for scaling or single-page mode, otherwise `canvas_w / 2`, integer division.
- If `int(scale * w)` is not greater than the usable width: keep it. No scrollbars.
- Otherwise:
  - Set `scale = usable width / w`.
  - Let `scrollbar_height = canvas_h - sb`.
  - If `scale * h` is greater than `scrollbar_height`: keep this width-fit scale. It is the larger of the two.
  - Otherwise: set `scale = scrollbar_height / h`. The page fills the height and needs a horizontal scrollbar.
- Apply the same neighbor check in two-page mode.

### 5.4 Oversize test

Used by fit-only-if-oversized. Compare oriented original size with the usable area:

| Zoom | Usable width | Usable height |
|---|---|---|
| Fit | `canvas_w` or half | `canvas_h` |
| Width | `canvas_w - sb` or half | `canvas_h` |
| Height | `canvas_w` | `canvas_h - sb` or half |
| Full, Custom | always oversized | always oversized |

"Half" applies to portrait pages in two-page mode. A square page counts as portrait here, even though it uses full width for scaling. This is a 0.8 quirk; match it or record a divergence. For the Height row, halve the usable height, not the width.

### 5.5 Full and custom

- Full: scale = 1. Do not resample. Draw the decoded page at 1:1.
- Custom: scale = percent / 100. Percent range is 1 to 200.

### 5.6 Scrollbars and pan

- If the scaled spread fits the viewport, suppress scrollbars and center it.
- If it does not fit, reserve `sb` and allow scrolling.
- Scroll step is 1 px. Arrow keys scroll 10 px.
- On a page change, horizontal scroll centers the spread and vertical scroll returns to the top.
- Drag panning moves the content with the pointer.

## 6. Resize filters

Use a separable two-pass convolution with an 8-bit intermediate buffer. Choose pass order:

- Horizontal first when `dst_w * src_h <= dst_h * src_w`.
- Vertical first otherwise.

For each destination pixel and axis:

- Let `scale = dst / src`.
- For minification (`scale < 1`): `fscale = scale`, `width = support / scale`.
- For magnification: `fscale = 1`, `width = support`.
- Center maps back as `center = dst_index / scale`.
- Source window runs from `floor(center - width)` to `ceil(center + width)`, clamped to valid source indices.
- Weight for source index `i`: `fscale * kernel(fscale * (center - i))`.
- Normalize weights so they sum to 1.
- Round the accumulated value with `+0.5`, truncate, clamp to 0..255.

Evaluate polynomial kernels in nested form, for example Mitchell
`p0 + x*x*(p2 + x*p3)`. Do not use `pow`. Evaluation order can change the last
bit, and the image vectors are exact hashes.

Kernels and support:

| Filter | Support | Kernel |
|---|---:|---|
| Box | 0.5 | 1 for `|x| <= 0.5`, else 0 |
| Bilinear | 1 | `1 - |x|` for `|x| < 1`, else 0 |
| Bicubic (Mitchell) | 2 | Mitchell-Netravali cubic with B = 1/3, C = 1/3 |
| B-spline | 2 | Cubic B-spline |
| Catmull-Rom | 2 | Cubic with B = 0, C = 1/2 |
| Lanczos3 | 3 | `sinc(x) * sinc(x/3)` for `|x| < 3`, else 0 |

Alpha is ignored. Decode to RGB, resize RGB, draw opaque.

Thumbnail rule:

- If `h / w > 0.6`: thumbnail height 60 px, width proportional.
- Otherwise: thumbnail width 100 px, height proportional.

## 7. Rotation and direction

### 7.1 Rotation

Rotation is per page and not persisted. Four states: 0, 90, 180, 270 degrees clockwise.

- Rotation is applied before landscape detection and before scaling.
- Rotate by index remapping, not interpolation.
- In single-page mode, rotate the visible page.
- For a landscape page split across the spread, rotate the page once.
- In a two-page portrait spread, rotate the left page or the right page separately.

### 7.2 Direction

LTR draws the spread's left page on the screen left. RTL draws it on the screen right.

For a split landscape page, the half selection must cancel the side swap:

- In LTR, the left half is the image's left half.
- In RTL, the left half is the image's right half.
- The right half is the opposite.

As a result, a split image is never mirrored, and page order reverses.

Labels follow the screen side: in RTL, the left page's label is on the right.

## 8. Rendering and layout

- Canvas background is black.
- Loading text is white and centered: "Page n loading...".
- Single page: centered horizontally, top-aligned.
- Double page: left page's right edge and right page's left edge meet at the horizontal center. Both pages are top-aligned.
- Content size is the sum of visible page widths and the maximum height.
- Fullscreen hides menu bar, toolbar and thumbnail browser. Only pages remain.
- The window title shows the file name.
- Page labels use "n of N". In two-page mode, show the left page number on the left label and the right page number on the right label. In single-page or split-landscape mode, show one label.
- Toolbar: rotate left page counter-clockwise, rotate left page clockwise, first, previous turn, previous page, next page, next turn, last, rotate right page counter-clockwise, rotate right page clockwise. Disable until a book is open.
- Thumbnail browser: 110 px wide, white background, 60 px rows, 5 px horizontal and 3 px vertical margins. Thumbnails centered. Selection follows the current page. Clicking a row jumps to that page.

## 9. Input

| Input | Action |
|---|---|
| Page Up | Previous turn |
| Page Down | Next turn |
| Backspace | Previous page |
| Space | Next page |
| Home | First page |
| End | Last page |
| Arrow keys | Scroll 10 px |
| Left drag | Pan with the pointer |
| Right click | Context menu |
| Mouse wheel | Scroll vertically (new behavior) |
| Ctrl + wheel | Zoom in or out (new behavior) |
| Alt+O | Open |
| Alt+X | Exit |
| Alt+Enter | Toggle fullscreen |
| F1 | About |

Context menu: open, open directory, previous page, next page, previous turn, next turn, first, last, rotate left page, rotate right page, fullscreen. Rotate targets the page under the pointer in two-page portrait mode.

## 10. Settings

Persist in a versioned TOML file in the platform config directory. Write atomically.

| Key | Type | Default |
|---|---|---|
| cache_bytes | integer | 805306368 |
| resample_bytes | integer | 268435456 |
| zoom | enum | width |
| zoom_percent | integer | 100 |
| fit_only_oversize | boolean | false |
| filter | enum | catmull_rom |
| mode | enum | two_page |
| direction | enum | ltr |
| window_width | integer | 600 |
| window_height | integer | 400 |
| window_x | integer | 50 |
| window_y | integer | 50 |
| toolbar_visible | boolean | true |
| browser_visible | boolean | true |
| last_directory | string | empty |
| natural_sort | boolean | false |
| exif_orientation | boolean | false |
| remember_page | boolean | false |

`remember_page` stores the last page per book. The storage format is implementation-private; it must be bounded so the settings file cannot grow without limit.

Optional one-time migration from old Comical settings is allowed, but the new app must start with its own defaults if migration is declined.

## 11. Errors and passwords

- Missing or unreadable file: show an error, do not open a book.
- Unsupported extension: show "Cannot open <file>".
- No pages: show "This archive does not contain any pages."
- Corrupt archive: show a clear error. Do not crash.
- Page decode or extraction failure: show a per-page error, keep the book open, display a placeholder.
- Encrypted archive: prompt for a password at open. Cancelling aborts the open.
- Wrong password: show a clear error and allow retry or cancel.
- Never persist passwords.
- Cap entry size and decoded pixel count before allocation. Over the cap is a page error, not a crash.

## 12. Archive behavior

- ZIP: store and deflate, ZIP64, ZipCrypto and AES-256. Decode names with the UTF-8 flag, otherwise CP437.
- RAR: RAR 1.4 through 7, solid and encrypted, including encrypted headers. Read-only.
- 7z: LZMA, LZMA2, BCJ, Delta, solid and AES-256. Read-only.
- Directory: recursive, same filter and sort.
- Never write to the archive or extract to disk.
- Multi-volume archives are out of scope for v1. Reject them as unsupported. Never return an incomplete page list.

## 13. Acceptance

The implementation is accepted when:

- Spread vectors for N = 1..12 and every landscape pattern pass.
- Zoom vectors for portrait, landscape, square, tiny and huge pages pass.
- Image hashes match for all six filters at scales 0.1, 0.25, 0.5, 0.75, 1.0, 1.3 and 2.0.
- ZIP, RAR and 7z corpus archives list, extract and render.
- Encrypted variants prompt and fail cleanly on wrong passwords.
- No UI freeze while opening or turning pages.
- No panic on corrupt archives or images.
