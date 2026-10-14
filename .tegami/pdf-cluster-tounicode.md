---
packages:
  "takumi-pdf": patch
---

# Copy text correctly from PDF in viewers that ignore `/ActualText`

When several glyphs draw one cluster, such as `S` and an acute for `Ś`, or a Devanagari consonant and its matra, each glyph now maps to its own characters in the font's ToUnicode table. Before, each mapped to the whole cluster, so pdf.js (Firefox), pypdf, and pdfminer read `Świnoujście` as `ŚŚwinoujśŚcie`, `मोटा` as `मोमोटाटा`, and a plain `S` elsewhere in the document as `Ś`.
