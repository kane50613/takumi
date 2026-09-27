---
packages:
  "takumi-pdf": patch
---

# Shadow colour emoji in PDF output

`text-shadow` on colour glyphs, such as COLR or bitmap emoji, now draws their silhouette in the shadow colour, as Chrome does. Before, PDF output drew no shadow for them.
