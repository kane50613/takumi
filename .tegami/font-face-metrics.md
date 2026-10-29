---
packages:
  takumi-core: minor
---

# `@font-face` applies `size-adjust` and the metric overrides

A face scales its glyphs and metrics by `size-adjust`, and `ascent-override`, `descent-override`, and `line-gap-override` replace the metrics the font file gives, as fractions of the font size, as Chrome applies them.
