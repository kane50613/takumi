---
packages:
  "@takumi-rs/core":
    type: patch
---

### Composite opaque effect layers without resampling them

A blur, shadow or opacity group that is fully opaque and composites normally is copied onto its parent row by row instead of going through the sampling pipeline. Such a drop shadow is about 40% faster to render, such a blur about 30%. A group with partial opacity or a blend mode is unaffected.
