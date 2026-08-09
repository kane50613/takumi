---
packages:
  takumi-core:
    type: minor
  takumi-svg:
    type: patch
  takumi-pdf:
    type: patch
  takumi-raster:
    type: patch
---

### Place replaced content from one place

`takumi_core::layout::replaced` sizes an image for its content box and places it. Each backend had worked out `object-fit` and `object-position` for itself, down to two byte-identical copies of the same position helper and a third spelling it out keyword by keyword.
