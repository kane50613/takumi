---
packages:
  takumi-core:
    type: minor
  takumi-svg:
    type: patch
  takumi-pdf:
    type: patch
---

### Shade a 3D border in every backend

`inset`, `outset`, `groove` and `ridge` borders now shade their sides in the SVG and PDF backends, as the raster backend already did.
