---
packages:
  takumi-core:
    type: minor
  takumi-svg:
    type: patch
---

### Draw every border ring from the shared painter

The SVG backend strokes and fills its borders through `takumi_core::painter::paint_border`, the same code the PDF backend runs. A `double` border now fills two rings instead of stroking two centerlines, matching the raster backend's geometry.

`border_paint` no longer reports a plain ring for a border that mixes a dashed or dotted side with solid ones. Filling the ring painted that side solid.
