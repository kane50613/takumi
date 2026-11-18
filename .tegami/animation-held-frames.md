---
packages:
  takumi-raster: patch
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
---

# Animated PNGs and GIFs are smaller and encode faster when frames repeat

Each frame of an animated PNG now stores only the pixels that changed since the frame before it, and a frame that changes nothing stores one pixel. An animated GIF stores an image held across several frames once, shown for their total delay, so it has fewer frames. Both decode to the same images at the same times as before.
