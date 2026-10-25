---
packages:
  takumi-core: patch
---

# Support `vector-effect: non-scaling-stroke`

An SVG shape with `vector-effect: non-scaling-stroke` now strokes its outline in the space of the SVG's viewport, as in Chrome, so the stroke keeps its width however the shape is transformed, and its markers scale with that width.
