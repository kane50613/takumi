---
packages:
  takumi-core:
    type: minor
  takumi-svg:
    type: patch
---

### Paint a background colour from one place

`takumi_core::paint_device` decides what a box's `background-color` paints and what shape it paints into. The raster, SVG, and PDF backends implement `fill_shape` and nothing else.

A rounded background in SVG output is now one `<path>` instead of a `<clipPath>`, a `<g>`, and a `<rect>`. The fill carries the shape, so it no longer needs the clip around it.
