---
packages:
  "takumi": patch
---

# Render rotated and scaled text faster

Glyphs under a rotation, scale or `offset-path` now come from the glyph cache, so repeated letters are rasterized once. Their origins snap to a quarter pixel the way Chrome's text rendering snaps them, which moves glyph edges by at most an eighth of a pixel.
