---
packages:
  "@takumi-rs/core":
    type: minor
---

### Support `border-collapse: collapse`

Neighbouring cells each painted their own border either side of the `border-spacing` gap. A collapsing table now resolves every shared line once: the wider border wins, `hidden` clears the line, and a row's borders reach its cells the way its background already did.
