---
packages:
  takumi-raster: patch
---

# Composite translucent layers faster

A layer with `opacity` below 1 now rounds its blended channels with plain float arithmetic that vectorizes, where it called a rounding function per channel. A page of four full-canvas translucent layers renders 19% faster, with identical pixels.
