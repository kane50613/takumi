---
packages:
  "takumi-paint": minor
  "takumi": minor
---

# Export what the renderer paints from JSX, HTML, or a node tree

`paint()` and `Painter.paint()` return a `PaintTree`: `nodes` in document order with their shapes, paints, glyph runs, and images resolved from CSS, and `steps` to draw them in paint order. A text node reports its resolved `textAlign`, and each run carries its `font`, `lineHeight`, and `letterSpacing`.
