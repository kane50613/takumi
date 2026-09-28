---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Lay out inline boxes as Chrome does

- A span's background and outline cover the font's ascent and descent instead of the whole `line-height`. A wrapped outline joins its lines only where they touch.
- A span's background and outline stop at the last glyph before a line break, like its text decoration.
- A span's left and right margins push the text beside it apart, on the parent's background. Negative margins still reserve nothing.
