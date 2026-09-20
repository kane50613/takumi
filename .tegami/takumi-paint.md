---
packages:
  "takumi-paint": minor
  "takumi": minor
---

# Export painted values from JSX, HTML, or a node tree

`paint()` and `Painter.paint()` return used decorations, image placement, and shaped text in paint order. A node carries `x`, `y`, `background`, `border`, `shadows`, `outline`, and `textRuns`, and each run carries its `font`. `walk()`, `textRuns()`, and `find()` traverse the tree.
