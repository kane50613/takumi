---
packages:
  takumi-raster:
    type: patch
---

### Crop `object-fit: cover` the way the other backends do

A length `object-position` under `cover` or `none` now shifts the image the same direction as the SVG and PDF output. Keyword and percentage positions are unchanged.
