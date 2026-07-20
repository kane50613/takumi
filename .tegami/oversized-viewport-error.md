---
packages:
  "cargo:takumi-raster": patch
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
---

### Return an error for viewports too large to allocate

A viewport whose pixel buffer overflowed the backing allocation used to fall back to a 1x1 canvas, so the render produced a valid-looking but wrong tiny image with no error. The root canvas is now built through a fallible path that surfaces the allocation failure as an `InvalidViewport` error instead. Internal offscreen canvases keep their bounded sizes and are unaffected.
