---
packages:
  "takumi-core": minor
---

# Lay out a node's inline content in one call

- `OwnContent::lay_out_inline` returns a `PaintedInline` with the node's spans, text and runs. Painting the node again in the same content box reuses it, unless an ellipsis cut the content.
- `PositionedInlineRun::resolved_glyphs` is a `ResolvedGlyphs`, an `FxHashMap` keyed by glyph id. Code that only calls `get` keeps working.
