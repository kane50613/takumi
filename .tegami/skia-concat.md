---
packages:
  takumi-core: patch
---

# Nested rotated and skewed transforms land closer to Chromium

Content under nested transforms that rotate or skew now lands closer to where Chromium puts it. In raster output a few antialiased edges shade one level differently, and in SVG output a `matrix(1 0 0 1 …)` group can appear or disappear.
