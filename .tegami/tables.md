---
packages:
  "takumi-core": patch
  "takumi": patch
  "takumi-pdf": patch
---

# Lay out tables as Chrome does

- An `auto`-width table shrinks to fit its content.
- Columns share the width through the CSS table algorithm. Percentage, `min-width` and `max-width` cell widths size their columns.
- `width: min-content`, `max-content`, `fit-content` and `stretch` size a table from its columns.
- `thead`, `tbody` and `tfoot` default to `vertical-align: middle`, as in Chrome.
- `vertical-align: baseline` cells line up on the row's deepest baseline.
- A non-cell inside a row gets wrapped in an anonymous cell.
- Tables with more than 10,000 rows lay out every row. Rows past the 10,000th used to pile up.
