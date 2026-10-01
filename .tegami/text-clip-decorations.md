---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Show a `background-clip: text` span's background through its text decorations

A span with `background-clip: text` now shows its background through its underlines, overlines and line-throughs as well as its glyphs, even when the decoration color is transparent, as in Chrome. Before, the decorations showed nothing.
