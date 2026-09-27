---
packages:
  "takumi-pdf": patch
---

# Paint inline blocks with no height

An `inline-block` with `height: 0` now paints the content that overflows it, as the raster output does.
