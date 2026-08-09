---
packages:
  takumi-pdf:
    type: patch
---

### Keep a clipped background out of the text layer

`background-clip: text` drew the run twice, once to fill the background through the glyphs and once for the text itself. Both landed in the text layer, so extraction, search and copy returned the text doubled. The background pass now paints the glyph outlines, which cover the same pixels without adding a second run of text.
