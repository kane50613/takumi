---
packages:
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
---

# A render aborted while its `fonts` list resolves starts no image loader

When the `signal` aborts while a promised `fonts` list is still resolving, the render now rejects as soon as the list resolves, without calling any image's `data()` loader. Before, it called each loader and then rejected.
