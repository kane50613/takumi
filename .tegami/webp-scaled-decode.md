---
packages:
  "cargo:takumi-core": patch
---

### Decode downscaled WebP at the target size

Native WebP sources drawn smaller than their pixel size now decode through libwebp's `use_scaling`, so the full-size frame is never allocated (~6 MB instead of ~48 MB for a 12 MP hero). libwebp's rescaler replaces the in-house resampler on this path, which can shift pixels slightly; wasm keeps the full-decode path.
