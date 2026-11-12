---
packages:
  takumi-core:
    type: patch
  takumi-pdf:
    type: patch
---

# A render decodes a repeated inline image once

An image whose source is a `data:` URI or SVG markup now decodes once while the render keeps reading it, in place of each time layout measures or paints it. The render keeps the 16 sources it read most recently, so a document of all-different images holds no more memory than before. An 11,000-row table with two SVG `data:` images per row renders to PDF in 16.1 s in place of 16.7 s, with the same bytes.
