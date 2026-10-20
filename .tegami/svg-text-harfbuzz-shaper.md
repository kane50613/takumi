---
packages:
  takumi-core: patch
---

# Match and shape SVG text fonts as HTML text does

SVG `<text>` now picks fonts with the CSS font matching HTML text uses, falls back per character through the same font list, and shapes through the same shaper. `font-variant="small-caps"` now synthesizes small capitals for a font without them. `font-weight`, `font-style`, and `font-stretch` pick faces and variation axes as CSS does.
