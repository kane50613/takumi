---
packages:
  takumi: patch
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
---

# Blur what lies past the canvas edge

Since 2.15.0, an element with a pixel-moving filter such as `blur()` that reached past the canvas's top or left edge lost that part before filtering, so its blur faded toward the edge. The filter layer now takes in what the filter reads past the canvas, as Chrome's compositor does.
