---
packages:
  "cargo:takumi-core": patch
---

### Route vendored resvg image decoding through the core decoders

SVG-embedded raster images now decode through the shared image pipeline, dropping the imagesize and zune-jpeg dependencies and tiny-skia's png-format feature.
