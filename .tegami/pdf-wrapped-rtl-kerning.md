---
packages:
  takumi-pdf: patch
  takumi-core: patch
---

# PDF output no longer panics on wrapped right-to-left text in fonts that kern the space glyph

A right-to-left paragraph that wrapped onto a second line could panic with `byte index is not a char boundary` when its font kerned the space glyph against letters. The panic is fixed. How the PDF's text reads when copied is not part of this change.
