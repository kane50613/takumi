---
packages:
  "takumi-paint": minor
  "takumi-core": minor
---

# Read a paint tree's text one glyph outline at a time

`TextRun.glyphOutline(index)` returns one glyph's outline as SVG path data, in the same space as `outline()`, so a renderer can draw, animate, or hit-test each letter on its own. In Rust, `PaintTree::glyph_outline` does the same.
