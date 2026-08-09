---
packages:
  takumi-core:
    type: minor
---

### Resolve background layers once, for every backend

`takumi_core::layout::background` works out how many tiles a `background-image` layer paints and where each one goes. The raster and SVG backends each carried a copy of that arithmetic. Rasterizing a tile stays with the backend that draws pixels.
