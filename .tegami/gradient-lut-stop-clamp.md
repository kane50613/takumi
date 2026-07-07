---
packages:
  "cargo:takumi-core": patch
  "cargo:takumi": patch
  "npm:@takumi-rs/core": patch
  "npm:@takumi-rs/wasm": patch
---

### Fix gradient stop snapping panic for oversized stop lists

Gradients with more color stops than the LUT can hold no longer panic. The
sample-index clamp and normalization passes stay within LUT bounds.
