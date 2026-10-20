---
packages:
  takumi-core: patch
---

# Draw bitmap emoji in SVG text

SVG `<text>` now draws bitmap glyphs, such as Noto Color Emoji's, in their own colors and the text's `fill-opacity`, as HTML text does. These glyphs used to be left out.
