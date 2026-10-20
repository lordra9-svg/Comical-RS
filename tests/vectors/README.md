# Test vectors

Vectors are data. They contain no reference code.

## Status

The current spread and zoom vectors are seed vectors, derived from
`spec/behavior.md`. They are not yet confirmed against the 0.8 binary. Mark new
vectors with `"status": "oracle-confirmed"` only after the oracle runner
records them.

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
- `expected`: `scale` with tolerance, and integer `width` and `height`.

## Images

`images.json` will hold input hashes, filter, target size and output hashes.
The oracle runner fills it. Do not put reference code here.

## UI

`ui.json` will hold labels, toolbar states and error messages. The oracle runner
fills it.
