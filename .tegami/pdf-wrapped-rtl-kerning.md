---
packages:
  takumi-pdf: patch
  takumi-core: patch
---

# PDF output no longer panics on wrapped right-to-left text in fonts that kern the space glyph

Rendering a PDF used to panic with `byte index is not a char boundary` when a right-to-left paragraph, such as Arabic text, wrapped onto a second line in a font that kerns the space glyph against letters. Justified paragraphs were also affected. These paragraphs now render.
