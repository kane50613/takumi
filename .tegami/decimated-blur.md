---
packages:
  takumi-core: patch
---

# Blurs and shadows render faster

`blur()`, `backdrop-filter`, `drop-shadow()`, `feGaussianBlur`, `box-shadow`, and `text-shadow` blur by decimation, ported from filtra, instead of with Skia's three-box passes. Their pixels no longer match Chrome byte for byte. They differ by a few levels out of 255 and lie closer to the exact Gaussian.
