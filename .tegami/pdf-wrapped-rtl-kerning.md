---
packages:
  takumi-pdf: patch
  takumi-core: patch
---

# PDF output no longer panics on wrapped right-to-left text in fonts that kern the space glyph

A right-to-left paragraph that wrapped onto a second line could panic with `byte index is not a char boundary` when its font kerned the space glyph against letters. Each glyph now maps to its own text, so the PDF's copied and searched text matches the source.
