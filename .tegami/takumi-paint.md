---
packages:
  "takumi-paint": minor
---

# Export what the renderer paints from JSX, HTML, or a node tree

`renderPaintTree()` and `PaintTreeRenderer.render()` return a `PaintDocument`: nodes in document order with their shapes, paints, glyph runs, and images resolved from CSS, and `paintSteps()` to draw them in paint order.
