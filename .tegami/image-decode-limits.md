---
packages:
  "cargo:takumi-core": patch
  "cargo:takumi": patch
  "npm:@takumi-rs/core": patch
  "npm:@takumi-rs/wasm": patch
---

### Cap image decode dimensions and GIF frame volume

Decoders reject images beyond 8192x8192 (via `image::Limits` for PNG and JPEG,
dimension checks for WebP) and GIFs beyond a total-frame pixel budget, stopping
decode-bomb OOM.
