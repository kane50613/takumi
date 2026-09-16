---
packages:
  "takumi": minor
---

# Size an SVG without the renderer

The `svg-sizing` feature reads an SVG's `width`, `height` and `viewBox` following usvg's rules, so a build that only lays out SVG images can leave the renderer out. `svg` implies it.
