---
packages:
  takumi-core: minor
---

# `@font-face` applies `size-adjust`, the metric overrides, and `font-feature-settings`

A face scales its glyphs and metrics by `size-adjust`, and `ascent-override`, `descent-override`, and `line-gap-override` replace the metrics the font file gives, as fractions of the font size. Its `font-feature-settings` turn on features for text in that face, except a feature the `font-feature-settings` property also sets, which the property decides. Each works as it does in Chrome.
