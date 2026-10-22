---
packages:
  takumi-core: patch
  takumi: patch
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
---

# Reject a GIF frame larger than the dimension limit

A GIF frame wider or taller than 8192 pixels now fails to decode, like a GIF whose canvas is that large. A GIF of a few dozen bytes could declare a 1×1 canvas holding a 65535×65535 frame, and decoding it allocated 17 GB.
