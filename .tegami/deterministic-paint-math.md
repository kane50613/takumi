---
packages:
  "cargo:takumi-core": patch
  "cargo:takumi-raster": patch
  "cargo:takumi-svg": patch
---

### Render identically on every platform

Replace libm trigonometry in the painting paths with deterministic polynomial implementations, so macOS and Linux produce byte-identical output. Conic gradients sample angles with Skia's `xy_to_unit_angle` polynomial instead of a per-pixel `atan2`, which is also faster.
