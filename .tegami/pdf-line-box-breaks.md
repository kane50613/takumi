---
packages:
  "takumi-core": minor
  "takumi-pdf": patch
---

# Break pages between line boxes, as Chrome does

A page break now falls between two line boxes instead of between the glyph bands of a font whose ascent and descent outgrow its line height. A paragraph taller than a page used to leave the first page blank when a margin preceded it, and short paragraphs never split across pages. Both now paginate like Chrome's print output.
