---
packages:
  "cargo:takumi-raster": patch
---

### Stop parking retired subcanvas pixmaps for the rest of the render

A canvas held on to up to eight full-size pixmaps that isolated groups had finished with, so a page with several stacking contexts kept multiples of its own viewport alive until the render returned. They are plain allocations now, freed as each group composites, which is where the rest of the scratch buffers already went.
