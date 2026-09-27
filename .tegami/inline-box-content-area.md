---
packages:
  "takumi": patch
---

# Size inline backgrounds and outlines to the font, not the line height

A span's background and outline now cover the font's ascent and descent, as browsers draw them, instead of the whole `line-height`. An outline that wraps joins its lines only where they touch, so lines spaced apart get separate outlines.
