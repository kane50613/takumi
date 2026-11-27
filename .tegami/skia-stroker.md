---
packages:
  takumi-core: minor
  takumi-raster: patch
---

# Strokes and dashes are outlined as in Chromium

Strokes, dashed and dotted borders, and SVG strokes now match Chromium's outlines. A curve that doubles back on itself gets a rounded point where it turns, a dashed line of no length draws its round or square cap, and the edges of round caps and joins can shift by a few colour levels.

`takumi_core::path` gains `Path::stroke`, `Path::dash`, `Stroke`, and `StrokeDash`.
