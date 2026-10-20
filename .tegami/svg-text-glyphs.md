---
packages:
  takumi-core: patch
---

# Draw synthetic bold, synthetic italic, and color emoji in SVG text

SVG `<text>` now draws the synthetic bold and italic HTML text gets for a face without that weight or style, and draws COLRv0 color glyphs such as Twemoji in color. These glyphs used to draw upright, at regular weight, or not at all.
