---
packages:
  "npm:@takumi-rs/core": patch
  "npm:@takumi-rs/wasm": patch
---

### Let cached font buffers be garbage-collected

The renderer cached each registered font by its buffer in a `Map`, pinning the
data for the renderer's lifetime even after the caller dropped its reference.
Buffers now live in a `WeakMap`, so they are freed once nothing else holds them.
