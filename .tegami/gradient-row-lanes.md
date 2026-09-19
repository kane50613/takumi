---
packages:
  "takumi": minor
---

# Render oblique linear gradients faster

Opaque, non-repeating linear gradients at an angle fill four rows at a time, computing their LUT indices with NEON, SSE2, or wasm simd128.
