---
packages:
  takumi-pdf: patch
  takumi-core: patch
---

# Wrapped right-to-left text in fonts that kern the space glyph renders correctly

When a right-to-left paragraph, such as Arabic text, wrapped onto a second line in a font that kerns the space glyph against letters, PDF output panicked with `byte index is not a char boundary`, and justified lines in images spread their space to the wrong places or not at all. Both now render as they do without kerning.
