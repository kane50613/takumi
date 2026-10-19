---
packages:
  takumi-core: patch
  takumi-pdf: patch
---

# Shape with HarfBuzz 14.5.1

Shaping moves to harfrust 0.14, which matches HarfBuzz 14.5.1: Bengali ra-phala such as গ্র joins again, marks attach correctly after a ligated multiple substitution, and Hangul runs apply `calt` to the characters around them. A PDF now draws a glyph from a font's SVG table only when that glyph has SVG data.
