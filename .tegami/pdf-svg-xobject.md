---
packages:
  takumi-core:
    type: patch
  takumi-pdf:
    type: patch
---

# A PDF draws a repeated SVG image once, unless a color filter recolors it

An `<img>` of an SVG that the document shows more than once at one size and in one color now lays out and draws as a form XObject that every later copy invokes. An SVG shown once, or under a `filter` that recolors it, draws in place as before. A document that repeats an icon on every row renders faster and writes a far smaller PDF. An 11,000-row table with two SVG images per row renders in 13.9 s in place of 16.6 s, and its PDF shrinks from 28.9 MB to 12.7 MB, with every page drawn the same.
