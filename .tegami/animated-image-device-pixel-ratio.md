---
packages:
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
  takumi-core: minor
  takumi-raster: patch
---

# GIFs, APNGs, and animated WebPs draw at the device pixel ratio, and as backgrounds

With a `devicePixelRatio` above 1, an animated image drew at its CSS size in device pixels. It filled the top-left of its box, and its right column and bottom row streaked across the rest. A single-frame GIF drew the same way. An animated image used as a `background-image` drew nothing at all. Both now draw as a still PNG does.

`ImageSource::dimensions` gives an image's natural size in its own pixels.
