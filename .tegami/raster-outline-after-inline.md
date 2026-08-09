---
packages:
  takumi-raster:
    type: patch
---

### Paint an outline above the text it wraps

A box with a negative `outline-offset` drew its own text over the outline ring. The outline now paints after everything inside the box, matching the SVG and PDF backends.
