---
packages:
  takumi-core:
    type: minor
---

### Build gradient tables through ColorLut

Gradient tiles carry one `lut: ColorLut`. `ResolvedGradientStop::resolve` and `ColorLut::new` replace the free functions.
