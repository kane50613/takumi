---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Fit text with `text-fit` as Chrome does

- Fixed `letter-spacing` and `word-spacing` keep their size when `text-fit` scales a line, and a fixed `line-height` keeps its height around the scaled glyphs.
- `-webkit-text-stroke`, `text-shadow` and text decorations scale with the line.
- A line whose fixed `letter-spacing` or `word-spacing` already fills its box stays unscaled, as in Chrome, where it used to vanish.
