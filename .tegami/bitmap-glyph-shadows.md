---
packages:
  "takumi": patch
---

# Cast text shadows from bitmap emoji

Color bitmap glyphs, such as Noto Color Emoji, now cast their `text-shadow` in image and SVG output. The shadow follows the glyph's shape. Before, those glyphs cast no shadow.
