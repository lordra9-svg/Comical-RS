# Test vectors

Vectors are data. They contain no reference code.

## Status

The current spread and zoom vectors are seed vectors, derived from
`spec/behavior.md`. They are not yet confirmed against the 0.8 binary. Mark new
vectors with `"status": "oracle-confirmed"` only after the oracle runner
records them. `images.json` is oracle-confirmed and marked as such.

## Spreads

`spreads.json` cases use:

- `pages`: array of `"portrait"` or `"landscape"`, after rotation.
- `start`: spread before the operation, or `null`.
- `op`: `first`, `last`, `next_turn`, `prev_turn`, `next_page`, `prev_page`,
  or `goto` with a `page` field.
- `expected`: spread in book order. `right` may be `null`.
- `part`: `full`, `left`, or `right`.

RTL does not change the book-order spread. It changes which page is drawn on
the screen left. Test that separately.

## Zoom

`zoom.json` cases use:

- `canvas`: width and height in physical pixels.
- `scrollbar`: scrollbar thickness.
- `mode`: `one_page` or `two_page`.
- `page`: oriented page width and height.
- `zoom`: `fit`, `width`, `height`, `full`, or `custom`.
- `zoom_percent`: used by `custom`.
- `fit_only_oversize`: boolean.
- `neighbors`: optional array of page sizes that share the two-page layout. Used by fit-width and fit-height neighbor checks.
- `expected`: `scale` with tolerance, and integer `width` and `height`.

## Images

`images.json` is oracle-confirmed. It contains:

- `source`: `width`, `height`, `pixel_rule`, `rgb_hex` and `rgb_sha256`.
  `rgb_hex` is row-major RGB8, three bytes per pixel, with no alpha.
- `cases`: `name`, `filter`, `target` (`width`, `height`) and `output_sha256`.

Filters are `box`, `bicubic`, `bilinear`, `bspline`, `catmull_rom` and
`lanczos3`. The test reconstructs the source from `rgb_hex`, resizes to the
target with the named filter, and hashes the raw RGB8 output. Hashes are exact.

The vectors cover a 17x11 source, all six filters, scales from 0.1 to 2.0, odd
dimensions and both pass orders. The oracle harness is not in this repository.

## UI

`ui.json` will hold labels, toolbar states and error messages. The oracle runner
fills it.
