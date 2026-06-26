---
packages:
  "npm:@takumi-rs/core": patch
---

### Bound the SVG raster cache

The per-SVG rasterization cache is now size-capped instead of growing unbounded
for the lifetime of the SVG source.
