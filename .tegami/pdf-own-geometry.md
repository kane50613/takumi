---
packages:
  takumi-pdf: patch
---

# PDF form bounds around strokes follow the raster stroker

A form's bounding box around a stroke in PDF output now comes from the same stroker raster uses.

- Where a stroke runs along a curve, the box can move by a few thousandths of a point.
- A dashed stroke's box now covers the whole undashed outline, which holds every dash the viewer draws.
