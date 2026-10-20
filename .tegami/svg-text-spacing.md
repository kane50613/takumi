---
packages:
  takumi-core: patch
---

# Space SVG text as HTML text does

SVG `<text>` now applies `letter-spacing` and `word-spacing` with the same rules as HTML text. Nonzero `letter-spacing` turns off ligatures, and the spacing also follows the last letter, which moves text centered with `text-anchor="middle"`. `word-spacing` now applies to a leading space too, as Chromium does in SVG.
