---
packages:
  "takumi-pdf": patch
---

# Copy text correctly from PDF in viewers that ignore `/ActualText`

When several glyphs draw one character, such as `S` and an acute for `Ś`, each glyph now maps to its own character in the font's ToUnicode table. Before, each mapped to the whole cluster, so pdf.js (Firefox), pypdf, and pdfminer read `Świnoujście` as `ŚŚwinoujśŚcie`, and a plain `S` elsewhere in the document as `Ś`.
