---
packages:
  takumi-core: patch
  takumi-pdf: patch
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
---

# An image whose header claims too many pixels fails to load instead of aborting a PDF

A PNG or WebP whose header claims an edge longer than 8192 pixels, the limit every decode already enforced, now fails to load like any other undecodable image. A PDF used to embed such a PNG as is and abort the whole process when it sized a buffer from that header.
