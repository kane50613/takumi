---
packages:
  takumi-core: minor
  takumi-raster: patch
  takumi-pdf: patch
  takumi-svg: patch
---

# Draw SVG images at the size they take up

An SVG in an `<img>`, a background, or an inline `<svg>` now lays out at the size it is drawn at, as in Chrome, instead of drawing at its own size and stretching. A `viewBox` keeps its `preserveAspectRatio` in a box of another shape, viewport units and percentages measure that box, and `object-fit: cover` crops the SVG instead of squeezing it. An SVG without a `viewBox` still stretches, as in Chrome.
