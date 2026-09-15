---
packages:
  "takumi-paint": minor
---

# Add `takumi-paint`, a paint tree of used values

`renderPaintTree()` lays out JSX, HTML, or a node tree and returns every box's used background, border, shadows, outline, image placement, and shaped text runs in paint order, so a PPTX or Canvas exporter reads what the renderer painted instead of re-running the cascade.
