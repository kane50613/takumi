---
packages:
  "cargo:takumi-core": patch
---

### Feed filter layers to resvg without a PNG roundtrip

Hand the premultiplied layer pixels to the vendored resvg pipeline through a new raw image kind, dropping the unpremultiply + PNG encode/decode in `apply_svg_filter`.
