---
packages:
  "takumi": minor
---

# Draw glyph outlines through `draw_outline`

`base::resources::glyph::draw_outline` draws a skrifa `OutlineGlyph` through `&mut dyn OutlinePen`, the path skrifa's own bounds pen shares. `ErasedPen` is deprecated in its favour.
