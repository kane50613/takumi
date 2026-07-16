---
packages:
  "cargo:takumi-raster": patch
---

### Encode lossless WebP without a YUV420 round trip

Set `use_argb` on the imported picture so `WebPLossless` and lossless animated WebP keep their source pixels instead of passing through chroma subsampling.
