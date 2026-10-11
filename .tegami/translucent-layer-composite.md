---
packages:
  "takumi": patch
---

# Composite translucent layers faster

A layer with `opacity` below 1 now composites through a direct loop instead of tiny-skia's general pipeline, with the same pixels. Real cards with translucent overlays render up to 65% faster.
