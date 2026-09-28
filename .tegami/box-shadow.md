---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Paint `box-shadow` as Chrome does

- The first shadow in a list sits on top in SVG and image output, as it already did in PDF.
- A spread shadow follows the outset-adjusted border radius. A square corner stays square, and a small radius grows less than the spread.
- SVG no longer paints an outer shadow under a translucent box.
- A PDF outer shadow no longer leaves a hairline around the box, and no longer fills the box when the offset moves the shadow clear of it.
- The image output places shadows at fractional offsets instead of rounding them toward zero.
