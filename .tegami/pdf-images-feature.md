---
packages:
  takumi-pdf: minor
---

# The `raster-images` feature of takumi-pdf is folded into `images`

`images` now brings the PNG, JPEG, and WebP decoders itself. A build that enabled `raster-images` alone should enable `images` instead, since without it the PDF never drew the raster images those decoders read.
