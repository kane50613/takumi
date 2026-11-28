---
packages:
  takumi-core: minor
---

# Transforms and rects can key a hash map

`Affine` and `Rect<f32>` implement `Eq` and `Hash`, and `SvgSpreadMethod` implements `Hash`.
