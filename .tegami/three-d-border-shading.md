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

`inset`, `outset`, `groove` and `ridge` came out as one flat ring in the SVG and PDF backends. Only the raster backend lit the sides the way a browser does. All three now shade each side, and split `groove` and `ridge` into their two bands.

`takumi_core::layout::border::side_bands` returns the strips a side fills and the colour each one takes, so the three backends stopped deciding that separately.
