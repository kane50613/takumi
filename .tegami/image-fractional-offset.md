---
packages:
  takumi-raster: patch
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
---

# An image moved by a fraction of a pixel blends its edges

An image that a `transform` such as `translate(0.5px)` moved by part of a pixel, drawn at its own size, snapped to the nearest whole pixel and lost the shift. It now blends across the pixel it falls between, as a browser draws it.
