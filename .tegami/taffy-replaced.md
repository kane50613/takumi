---
packages:
  "takumi-core": patch
  "takumi": patch
---

# Keep images at their natural size in grids and between insets

An image or SVG in a grid cell no longer stretches to the cell under the default alignment, and an absolutely positioned one no longer stretches between its insets. Both keep their natural size, as Chrome draws them.
