---
packages:
  cargo:takumi-raster: minor
---

### Rename the animated encoders to `write_animated_*`

`encode_animated_gif`, `encode_animated_png`, and `encode_animated_webp` are
now `write_animated_gif`, `write_animated_png`, and `write_animated_webp`,
matching `write_image`. Signatures are unchanged.
