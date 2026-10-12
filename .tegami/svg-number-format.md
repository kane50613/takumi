---
packages:
  "takumi-core": patch
  "takumi": patch
---

# Write SVG output faster

SVG output and the paint tree's glyph outlines write their numbers without float formatting, with the same bytes. A text-heavy SVG renders about twice as fast.
