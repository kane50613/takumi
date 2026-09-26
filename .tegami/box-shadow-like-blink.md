---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Paint `box-shadow` the way Chrome does in every output format

- The first shadow in a `box-shadow` list now sits on top in SVG and raster output, as it already did in PDF.
- SVG no longer paints an outer shadow under a translucent box.
- A PDF outer shadow no longer leaves a hairline around the box, and no longer fills the box when the shadow is offset clear of it.
- Raster places shadows at fractional offsets instead of rounding them toward zero.
