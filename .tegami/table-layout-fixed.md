---
packages:
  "@takumi-rs/core":
    type: minor
---

### Support `table-layout: fixed` and `border-spacing`

`border-spacing` was a hardcoded 2px gap with no way to change it, and `table-layout` did nothing. Column widths now come from the first row under `table-layout: fixed` and the rest of the width is shared evenly, while `border-spacing` sets both the gap between cells and the inset from the table's own edges. On a three-column table measured against headless Chrome, `auto` columns land 67px to 136px away and `fixed` columns land within 2px.

`border-spacing` starts at 0 as CSS defines it, and the HTML and JSX element presets give `<table>` the 2px browsers apply, so a `display: table` box no longer gets a gap it never asked for.
