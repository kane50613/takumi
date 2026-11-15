---
packages:
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
---

# A render aborted while its `fonts` list resolves starts no image loader

When the `signal` aborted while a promised `fonts` list was still resolving, the render went on to call each image's `data()` loader before it rejected. It now checks the signal once the list resolves and rejects without starting them.
