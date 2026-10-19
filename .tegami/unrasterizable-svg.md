---
packages:
  takumi-core: patch
  takumi: patch
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
---

# Skip an SVG too large to rasterize instead of failing the render

An SVG image or background that would rasterize past the pixel budget, such as `background-size: cover` on an SVG with an extreme `viewBox`, is now left undrawn. The whole render used to fail with `InvalidPixmapSize`.
