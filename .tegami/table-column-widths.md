---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Size tables and their columns as Chrome does

A block-level `auto`-width table now shrinks to fit its content instead of filling its container. Columns share the table's width with the CSS table width algorithm, so a table narrower than its content no longer squeezes a column to its minimum. Percentage, `min-width` and `max-width` cell widths now constrain their columns instead of resizing the cell inside them.
