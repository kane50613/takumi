---
packages:
  "@takumi-rs/core":
    type: minor
---

### Collapse a table's borders onto shared lines

`border-collapse: collapse` had no effect, so neighbouring cells each painted their own border either side of the `border-spacing` gap. A collapsing table now resolves every shared line once: the wider border wins, `hidden` clears the line, and a row's borders reach its cells the way its background already did.
