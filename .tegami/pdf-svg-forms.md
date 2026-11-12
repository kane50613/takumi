---
packages:
  takumi-pdf:
    type: patch
---

# A PDF draws a repeated SVG image once, unless a color filter recolors it

An `<img>` of an SVG that a PDF shows more than once at one size and in one color now draws as a form XObject that every later copy invokes. An SVG shown once, or under a `filter` that recolors it, draws in place as before. A document that repeats an icon on every row writes a far smaller PDF.
