---
packages:
  "takumi-pdf": patch
---

# Draw dashed and dotted border sides next to other styles in PDF

A border that mixes styles, such as a dashed top over a solid bottom, used to leave its dashed and dotted sides out of the PDF. Those sides now draw with their pattern, the same way the SVG backend draws them.
