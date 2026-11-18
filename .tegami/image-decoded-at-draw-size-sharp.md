---
packages:
  takumi-raster: patch
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
---

# A cropped image decoded at its drawn size draws sharp

An image that `object-fit: cover` or `object-position` crops, decoded at the size it draws, could pass through the bilinear filter and come out slightly soft, as when a photo 4032 pixels wide draws 1200 wide. It now copies its pixels, and draws faster.
