---
packages:
  "@takumi-rs/wasm": patch
---

# A smaller WASM binary

The SVG writer behind `renderSvg` is now optimized for size in the WASM build, which cuts the binary by about 37 KB raw and 9 KB brotli. The native binary keeps its speed-optimized build.
