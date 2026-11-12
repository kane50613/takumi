---
packages:
  takumi-core:
    type: patch
  takumi-pdf:
    type: patch
---

# A PDF draws each SVG image once, however many times the page shows it

An `<img>` of an SVG now lays out and draws once per size, as a form XObject every later copy of it invokes. A document that repeats an icon on every row renders faster and writes a far smaller PDF: an 11,000-row table with two SVG images per row renders in 13.9 s in place of 16.6 s, and its PDF shrinks from 28.9 MB to 12.7 MB, with every page drawn the same.
