---
packages:
  cargo:takumi-css: minor
---

### Keep `image` and `tiny_skia` out of the `Color` API

The public `From<Color>` conversions to `image::Rgba` and
`tiny_skia::PremultipliedColorU8` (and back) are gone. The raster backend does
the conversions internally instead.
