---
packages:
  "takumi-core": minor
---

# Lay out a node's inline content for painting in one call

`OwnContent::lay_out_inline` lays a node's inline content out in its content box and returns a `PaintedInline`: its spans, its text and its resolved runs. Painting the node again in the same content box reuses the layout, unless `text-overflow: ellipsis` cut the content short.
