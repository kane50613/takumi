---
packages:
  "cargo:takumi-svg": patch
---

### Anchor element filter regions to the border box

An invisible rect keeps a filtered element's region from collapsing when nothing inside it paints, matching the raster backend for filter-driven overlays like `feTurbulence` grain.
