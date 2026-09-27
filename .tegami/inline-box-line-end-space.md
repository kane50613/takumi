---
packages:
  "takumi": patch
---

# Leave the space a line wraps at out of inline backgrounds and outlines

A span's background and outline now stop at the last glyph before a line break, like its text decoration, instead of covering the space the line wrapped at.
