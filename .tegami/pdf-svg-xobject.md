---
packages:
  takumi-core:
    type: patch
  takumi-pdf:
    type: patch
---

# A PDF draws a repeated SVG image once, however many times the pages show it

An `<img>` of an SVG that the document shows more than once at one size now lays out and draws as a form XObject that every later copy invokes. An SVG shown once draws in place as before. A document that repeats an icon on every row renders faster and writes a far smaller PDF: an 11,000-row table with two SVG images per row renders in 13.9 s in place of 16.6 s, and its PDF shrinks from 28.9 MB to 12.7 MB, with every page drawn the same.
