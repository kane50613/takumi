---
packages:
  "@takumi-rs/core":
    type: patch
---

### Move table cell content with `vertical-align`

`vertical-align: middle` and `bottom` on a table cell now move its content down the cell instead of leaving it at the top. `baseline` still renders as `top`. A row child that is not a `table-cell`, such as a `<td style="display: flex">`, is now laid out in the row instead of dropped.
