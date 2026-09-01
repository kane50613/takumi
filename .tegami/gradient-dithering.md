---
packages:
  "takumi-core":
    type: minor
  "takumi-raster":
    type: minor
---

### Dither gradients under the dithering option

With `dithering` set, gradient fills add ordered Bayer noise before 8-bit
quantization, as Chrome does, so slow ramps stop banding. The old output pass
that reduced every pixel to 128 levels is gone; `floyd-steinberg` is now a
deprecated alias of `ordered-bayer`. The default output is unchanged.
