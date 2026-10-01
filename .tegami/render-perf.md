---
packages:
  "takumi": patch
---

# Render faster

- Shadows, backdrop filters and `blur()` blur faster, with NEON, SSE2 or wasm simd128 on the alpha pass.
- Text is shaped and measured once per node, and glyph positions, `text-decoration-skip-ink` intercepts and `line-height: normal` metrics are reused within a render.
- Oblique linear gradients, scaled images, masks and the final alpha pass skip per-pixel work.
