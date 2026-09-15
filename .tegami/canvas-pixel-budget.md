---
packages:
  "takumi": minor
---

# Render canvases up to 64 megapixels

The canvas budget is now 64 megapixels, up from 16. A 7680 × 4320 banner or a 4096 × 16384 page fits.

Over the budget, `render` fails with `InvalidViewport`. The message now states the limit.
