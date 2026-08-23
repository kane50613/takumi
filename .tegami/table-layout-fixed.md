---
packages:
  "@takumi-rs/core":
    type: minor
---

### Support `table-layout: fixed` and `border-spacing`

`border-spacing` was a fixed 2px default with no way to change it, and `table-layout` did nothing. Column widths now come from the first row under `table-layout: fixed` and the rest of the width is shared evenly, and `border-spacing` sets the gap on both axes. On a three-column table measured against headless Chrome, `auto` columns land 67px to 136px away and `fixed` columns land within 2px.
