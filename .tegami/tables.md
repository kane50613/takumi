---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Lay out tables as Chrome does

- A block-level `auto`-width table shrinks to fit its content instead of filling its container.
- Columns share the table's width through the CSS table width algorithm, so a table narrower than its content no longer squeezes a column to its minimum. Percentage, `min-width` and `max-width` cell widths constrain their columns instead of resizing the cell inside them.
- The HTML presets give `thead`, `tbody` and `tfoot` Chrome's `vertical-align: middle`, which rows and cells inherit, and a cell that holds only text follows it.
- Cells with `vertical-align: baseline` line their first lines up on the row's deepest baseline, whatever their fonts, padding or borders.
- An element inside a row that is not a table cell sits in an anonymous cell, as CSS table fixup puts it.
- A table's `width: min-content`, `max-content`, `fit-content` and `stretch` size it from its column grid, where they used to act as `auto`.
