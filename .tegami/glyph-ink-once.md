---
packages:
  "takumi-core": minor
  "takumi": patch
  "takumi-pdf": patch
---

# Read each glyph's ink box once

A glyph's ink box is worked out when the glyph is resolved, not on every use. Large documents lay out about 7% faster.

`ResolvedOutlineGlyph::Plain` and `ResolvedOutlineGlyph::Color` gain an `ink` field, which `outline_ink` computes from their paths.
