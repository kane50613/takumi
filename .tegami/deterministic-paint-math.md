---
packages:
  "cargo:takumi-core": patch
---

### Sample conic gradients deterministically

Replace the per-pixel libm `atan2` in conic gradient sampling with Skia's `xy_to_unit_angle` polynomial, so every platform renders identical conic output and sampling gets cheaper.
