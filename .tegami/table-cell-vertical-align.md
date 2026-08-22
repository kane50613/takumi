---
packages:
  "@takumi-rs/core":
    type: patch
---

### Align table cell content with `vertical-align`

`vertical-align: middle` on a table cell now centers its content in the cell, and `bottom` pushes it to the cell's bottom edge. `baseline` still renders as `top`.

A row's element children that are not `table-cell`, such as a `<td style="display: flex">`, are now laid out in the row instead of dropped. Text sitting directly in a row is still dropped: it has no anonymous cell to go into.
