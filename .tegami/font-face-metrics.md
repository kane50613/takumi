---
packages:
  takumi-core: minor
---

# `@font-face` applies `size-adjust`, the metric overrides, `font-feature-settings`, and `local()`

A face scales its glyphs and metrics by `size-adjust`, and `ascent-override`, `descent-override`, and `line-gap-override` replace the metrics the font file gives, as fractions of the font size. Its `font-feature-settings` turn on features for text in that face, except a feature the `font-feature-settings` property also sets, which the property decides. A `local()` source draws with the registered font whose full name or PostScript name it gives, ignoring ASCII case, and moves on to the next source when no registered font has that name. Each works as it does in Chrome, with the registered fonts in place of system fonts.
