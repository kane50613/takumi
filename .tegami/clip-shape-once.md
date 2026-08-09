---
packages:
  takumi-raster:
    type: patch
---

### Resolve a `clip-path` shape from one place

The raster backend resolved `inset()`, `ellipse()`, `polygon()` and `path()` itself. An `ellipse()` took its keyword radii across both axes, so a non-square box got the wrong ones, and a percentage corner radius in `inset()` measured against the width on both axes.
