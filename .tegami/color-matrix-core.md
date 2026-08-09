---
packages:
  takumi-core:
    type: minor
  takumi-raster:
    type: patch
---

### Rotate hue from the same matrix everywhere

`takumi_core::filter::ColorMatrix` turns a colour-transforming `filter` function into the matrix Filter Effects defines for it. The raster backend had written the `hue-rotate` coefficients out a second time and rounded the angle to whole degrees first, so `hue-rotate(45.5deg)` rotated by 45.
