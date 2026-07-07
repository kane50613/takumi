---
packages:
  "cargo:takumi-raster": patch
  "cargo:takumi": patch
  "npm:@takumi-rs/core": patch
  "npm:@takumi-rs/wasm": patch
---

### Fix buffer pool bucket capacity invariant

Release now buckets a buffer by the floor power of two its capacity guarantees,
and `acquire_dirty` reserves before `set_len`. A pooled buffer can no longer be
lengthened past its allocation.
