---
packages:
  takumi-core: patch
  takumi-pdf: patch
---

# Long documents with a positioned box in a box broken across every page render in linear time

When an absolutely positioned box sat in a box broken across every page, and the box's containing block placed it on the last page, every page painted from the layout of the whole document. Rendering time grew with the square of the page count: a 2,000-row table took 49 seconds. Each page now paints from its own page and its neighbors, and the same table renders in 3 seconds, with the same PDF.
