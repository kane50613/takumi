---
packages:
  takumi-core: patch
  takumi: patch
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
---

# Apply the dimension limit to each GIF frame

A GIF frame wider or taller than 8192 pixels now fails to decode, as a GIF whose canvas is that large already does.
