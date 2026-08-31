---
packages:
  "takumi-core":
    type: minor
---

### Dither gradients under the dithering option

With `dithering` set, gradient fills add ordered Bayer noise before
quantizing to 8-bit, the way Skia dithers every Chrome gradient, so slow
ramps stop banding. The default stays noise-free: dithered gradients
compress poorly in lossless formats.
