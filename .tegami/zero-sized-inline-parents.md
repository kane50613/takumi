---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Paint sized text inside a zero-sized parent in SVG and PDF

Spans that set their own `font-size` inside a `font-size: 0` block now paint in SVG and PDF, and PDF paints text overflowing a box with no width or height, as the raster output does. PDF no longer writes `NaN` for a zero-sized run.
