---
packages:
  takumi-raster: patch
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
---

# Animated PNG and GIF write a held frame once

A frame of an animated PNG now replaces only the pixels that changed since the frame before it, and a frame that changes nothing writes one pixel. An animated GIF shows an image held across several frames as one frame for the whole time. The animation plays the same, and a timeline that holds its images encodes several times faster into a fraction of the bytes.
