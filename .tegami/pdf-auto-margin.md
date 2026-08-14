---
packages:
  takumi-pdf:
    type: minor
---

### Size the page margin to its band

A band draws inside the page margin, and a margin shorter than the band left content running underneath it. `margin` now takes `"auto"` on any side and starts there, growing to the space that side's band needs and never dropping below the 48 it began at.
