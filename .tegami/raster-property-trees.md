---
packages:
  "takumi": patch
---

# Keep filtered content inside the overflow clip it sits in

A blurred or otherwise filtered box inside an `overflow: hidden` parent no longer spills past the parent's edges, as in Chrome. The raster backend now enters each box's clips and effects the way Chrome's compositor does.
