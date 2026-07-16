---
packages:
  "cargo:takumi-raster": patch
---

### Raise the lossless WebP compression effort

Encode `WebPLossless` at effort 50 instead of 20, shrinking output by about 10% at 1.5x the encode time.
