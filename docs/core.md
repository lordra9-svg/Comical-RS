# Core layout and zoom

`comical-core` is pure geometry. It has no dependencies and performs no I/O.
It owns book-order page descriptions, the spread state machine and zoom math.

Source: `crates/comical-core`.

## Book-order model

```rust
pub struct PageEntry {
    pub name: String,
    pub size: u64,
}
```

`PageEntry` is defined in `src/lib.rs`. The core does not know archive formats.

## Spread state

### Types

```rust
pub enum PageOrientation { Portrait, Landscape }
pub enum PagePart { Full, Left, Right }
pub struct PageRef { pub page: usize, pub part: PagePart }
pub struct Spread { pub left: PageRef, pub right: Option<PageRef> }
pub enum ViewMode { OnePage, TwoPage }
```

Orientation is measured after rotation. A square page is portrait.

`Spread` is in book order:

- `left` is the left page in reading order, not necessarily the screen left.
- `right` is the facing page, if any.
- For right-to-left UI, swap screen sides at draw time. Do not swap these fields.

### State

```rust
pub struct SpreadState {
    pages: Vec<PageOrientation>,
    mode: ViewMode,
    spread: Spread,
}
```

`current_page()` returns `spread.left.page`, not the right page. This matters for
mode switches and for pages that span two spreads.

Construction:

- `new(pages, mode)` starts at the first spread. Empty book -> `EmptyBook`.
- `with_spread(pages, mode, spread)` restores a known spread. It checks bounds,
  `left.page <= right.page`, one-page mode consistency and left part full in
  one-page mode. It trusts right/left part values.

Navigation:

- `first`, `last`.
- `next_turn`, `prev_turn`: move by a spread.
- `next_page`, `prev_page`: move by one page.
- `go_to_page(page)`: clamps to `0..=last`.
- `set_mode(mode)`: keeps `current_page`; edge pages use first/last rules.
- `toggle_mode`.

All navigation clamps. None returns an error.

## Spread construction

Let `N` be the page count.

### First spread

- One-page mode or `N == 1`: page 0 full, no right.
- Page 0 landscape: page 0 left half + page 0 right half.
- Otherwise: page 0 full + page 1 (left half if landscape, otherwise full).

### Last spread

- One-page mode or `N == 1`: page `N-1` full, no right.
- Page `N-1` landscape: both halves of page `N-1`.
- Otherwise: page `N-2` on the left (right half if landscape, otherwise full),
  page `N-1` full on the right.

### Start spread at page `p`

- `p` landscape: both halves of `p`.
- `p` portrait: `p` full on the left, page `p+1` on the right if it exists
  (left half if landscape, otherwise full).

### Turn forward

Let `right` be the current right page.

1. If there is no right page, use the last spread.
2. If `right.page >= N-1`, use the last spread.
3. If `right.page == N-2` and `right.part != Left`, use the last spread.
4. If `right.part == Left` and `right.page` is landscape, show that page's right
   half on the left and page `right.page + 1` on the right.
5. Otherwise, construct a start spread after `right.page`.

The guard in step 3 is why a penultimate split landscape page can end the book as
`[N-2 right, N-1 full]`, while a mid-book split page advances normally.

### Turn backward

Let `left` be the current left page.

1. If `left.page == 0`, use the first spread.
2. If `left.page == 1` and `left.part != Right`, use the first spread.
3. If `left.part == Right`, keep that page on the right as its left half and put
   page `left.page - 1` on the left (right half if landscape, otherwise full).
4. Otherwise, let `q = left.page - 1`.
   - If `q` is landscape, show both halves of `q`.
   - If `q` is portrait, put `q` full on the right and page `q-1` on the left
     (right half if landscape, otherwise full). If `q-1` does not exist, use the
     first spread.

### Page forward

- If the right page is the left half of a landscape page, show both halves of it.
- Otherwise shift the window: old right becomes new left; the next page becomes
  the new right (left half if landscape, otherwise full).
- At the end, use the last spread.

### Page backward

1. If `left.page == 0`, do nothing. This is not the same as a turn backward,
   which goes to the first spread.
2. If `left.part == Right`, show both halves of that page.
3. Otherwise shift backward: old left becomes new right; the previous page
   becomes new left (right half if landscape, otherwise full).

### Mode switch

- Two-page to one-page: show `current_page()` full.
- One-page to two-page:
  - page 0 -> first spread
  - last page -> last spread
  - otherwise -> start spread at that page

## Spread invariants

- Indices are in `0..N`.
- `left` exists.
- `right` is absent only in one-page mode or for `N == 1`.
- If `right` exists, `left.page <= right.page`.
- If both references are the same page, it is landscape with `left=Left`,
  `right=Right`.
- A right page is full (portrait) or left half (landscape). It is never a right
  half of a different page.
- A left page that is a left half is paired with its own right half.
- A left page may be a right half when it is the second spread of a split page.

Generated tests check these for N = 1 through 12 and several landscape patterns.
They also check that every landscape page appears, and that split halves are
adjacent in the turn sequence.

## Zoom types

```rust
pub struct Size { pub width: u32, pub height: u32 }

pub enum ZoomMode { Fit, Width, Height, Full, Custom }

pub struct ZoomOptions {
    pub mode: ViewMode,
    pub scrollbar: u32,
    pub zoom: ZoomMode,
    pub custom_percent: u32,
    pub fit_only_if_oversized: bool,
}

pub struct ZoomResult {
    pub scale: f32,
    pub width: u32,
    pub height: u32,
}
```

`Custom` clamps the percentage to 1 through 200.

Scale is `f32`. Output dimensions truncate toward zero and are at least 1.

## Zoom pipeline

`compute_zoom(canvas, page, options)` calls
`compute_zoom_with_neighbors(canvas, page, options, &[])`.

1. If `fit_only_if_oversized` is on and `is_oversized` is false, return 1:1.
2. Otherwise compute a scale for the mode.
3. Convert to integer width and height.

Modes:

- `Fit`: fit the whole page inside the usable area.
- `Width`: fill usable width, adding a vertical scrollbar if needed.
- `Height`: fill usable height, adding a horizontal scrollbar if needed.
- `Full`: 1.0, no resample.
- `Custom`: percent / 100.

## Fit math

### Usable width

For `Fit` and `Width` scaling:

- one-page mode or `width >= height` (square is wide): full canvas width
- otherwise: `canvas_width / 2` (integer division)

### Fit

Let `usable_width` be the value above.

- If `canvas_width / canvas_height > page_width / page_height`: scale =
  `canvas_height / page_height`.
- Otherwise: scale = `usable_width / page_width`.

These are float divisions. The first branch uses the full canvas height and does
not subtract a scrollbar.

### Fit width

1. `usable_width` as above.
2. `width_scale = usable_width / page_width`.
3. If `width_scale * page_height <= canvas_height`: use `width_scale`, no
   scrollbar.
4. Otherwise `height_scale = canvas_height / page_height`.
5. Let `scrollbar_width = canvas_width - scrollbar`, halved for portrait pages in
   two-page mode. Square pages use the full scrollbar width because they are wide
   for scaling.
6. If `int(height_scale * page_width) > scrollbar_width`: keep `height_scale`,
   no scrollbar.
7. Otherwise scale = `scrollbar_width / page_width`, with a vertical scrollbar.

Step 6 truncates the width before comparing. This matters at the pixel boundary.

### Fit height

1. `height_scale = canvas_height / page_height`.
2. `usable_width` as above.
3. If `int(height_scale * page_width) <= usable_width`: use `height_scale`, no
   scrollbar.
4. Otherwise `width_scale = usable_width / page_width`.
5. Let `scrollbar_height = canvas_height - scrollbar`.
6. If `width_scale * page_height > scrollbar_height`: keep `width_scale`, no
   scrollbar.
7. Otherwise scale = `scrollbar_height / page_height`, with a horizontal
   scrollbar.

Step 6 uses an untruncated float product. Step 3 truncates. Keep those choices
unless a vector proves the contract changed.

### Neighbor-aware fit

`compute_zoom_with_neighbors` is used for two-page layout.

After a fit-width or fit-height base result:

- If the base has no scrollbar, and the mode is two-page, and any neighbor base
  has a scrollbar, use the scrollbar-present scale for the current page.
- Width fallback: `scrollbar_width / page_width`.
- Height fallback: `scrollbar_height / page_height`.

This can shrink the current page even when it fits on its own, because the shared
spread will reserve a scrollbar for the neighbor. The caller decides which
neighbors to pass. The behavior contract checks both adjacent pages; the app
layer should pass the pages that share the spread.

## Oversize test

`is_oversized` compares the original oriented page with a usable area.

| Zoom | Usable width | Usable height |
|---|---|---|
| Fit | `canvas_width` or half | `canvas_height` |
| Width | `canvas_width - scrollbar` or half | `canvas_height` |
| Height | `canvas_width` | `canvas_height - scrollbar` or half |
| Full, Custom | always oversized | always oversized |

"Half" applies to portrait pages in two-page mode. A square page counts as
portrait for oversize, so it gets the half allowance, even though it uses full
width for scaling.

Important: for the Height row, halve the height, not the width. This was a real
bug fixed in review and is covered by
`height_oversize_halves_the_usable_height_for_portrait_pages`.

## Square-page quirks

A square page is:

- `Portrait` for spread pairing (not split).
- `is_wide` true for scaling (`width >= height`), so full usable width and full
  scrollbar width.
- `is_portrait` true for oversize (`height >= width`), so half width/height in
  two-page oversize.

This asymmetry is intentional and matches the behavior contract. Do not
"simplify" it without updating the vectors and the spec.

## Boundary quirks

- Turn and page steps are not inverses at the first and last spread.
- Previous page at page 0 is a no-op. Previous turn at page 0 goes to the first
  spread.
- A right half at page 0 can be the left page after a page step; previous page
  keeps that spread, previous turn reopens the first spread.
- A penultimate split landscape page ends at `[right half, last page]`; a
  mid-book split page advances to the following pages.

These are covered by named cases in `spreads.json` and by
`spreads_generated.rs`.

## How to change spread or zoom behavior

1. Find the named vector case and the matching test.
2. Confirm the behavior spec says what you think.
3. If the spec is correct and the code is wrong, change the smallest helper and
   add a regression case.
4. If the spec is wrong, change the spec first, then the vector, then the code.
   Do not edit a vector just to make a test pass.
5. Run `cargo test -p comical-core` and the full workspace.
6. Update `docs/core.md` and `tests/vectors/README.md` if the contract changed.

## Core tests

- `tests/vectors.rs`: every JSON case, float tolerance 1e-6.
- `tests/spreads_generated.rs`: N = 1..12 invariants, turn sequence, boundary
  regressions, mode switches.
- `tests/zoom_generated.rs`: portrait, landscape, square, tiny, huge, all modes,
  custom clamping, oversize quirks and neighbor fallback.

The vectors are seed vectors unless marked oracle-confirmed. They are the current
contract, not proof against the original binary.
