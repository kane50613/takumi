---
packages:
  "takumi": patch
---

# Skip text and shapes outside the image

Glyphs, shapes and clip paths that fall wholly outside the image are no longer rasterized. Large rotated or oversized backgrounds of repeated text render faster.
