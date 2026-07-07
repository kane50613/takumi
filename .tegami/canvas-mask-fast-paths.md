---
packages:
  "cargo:takumi-raster": patch
  "cargo:takumi": patch
  "npm:@takumi-rs/core": patch
  "npm:@takumi-rs/wasm": patch
---

### Skip mask copies and per-pixel mask lookups in canvas fast paths

Borrow the combined constraint mask directly when it already matches the
canvas viewport instead of copying it per draw, and hoist mask lookups out of
the per-pixel blit loops. Output is unchanged.
