---
packages:
  "cargo:takumi-raster": patch
---

### Fix debug-build panic when drop-shadow hits a fully transparent element

`drop-shadow()` on an element with no visible pixels panicked with an integer underflow in debug builds. The bounds check now short-circuits before computing the empty region's size.
