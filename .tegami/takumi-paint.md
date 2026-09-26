---
packages:
  "takumi-paint": minor
  "takumi": minor
---

# Export painted values from JSX, HTML, or a node tree

`paint()` and `Painter.paint()` return used decorations, image placement, and shaped text in paint order. A node carries `x`, `y`, `contentBox`, `textAlign`, `background`, `border`, `shadows`, `outline`, and `textRuns`. Each run carries its `font`, `lineHeight`, and `letterSpacing`. The tree iterates its nodes in paint order and offers `textRuns()` and `find()`.
