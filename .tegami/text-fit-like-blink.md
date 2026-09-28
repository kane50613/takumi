---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Fit text with `text-fit` as Chrome does

Fixed `letter-spacing` and `word-spacing` now keep their size when `text-fit` scales a line, and a fixed `line-height` now keeps its height around the scaled glyphs. `-webkit-text-stroke` now scales with the text it outlines.
