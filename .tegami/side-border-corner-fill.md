---
packages:
  "cargo:takumi-svg": patch
  "cargo:takumi-pdf": patch
---

### Fill side border corners in vector backends

With per-side border colors and rounded corners, the SVG and PDF backends left the corner arcs unpainted: side fills used straight-edged polygons that stop short of the curve. Sides now fill with the same contour-following polygons the raster backend uses.
