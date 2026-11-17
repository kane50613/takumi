---
packages:
  takumi-core: patch
  takumi-pdf: patch
---

# Long PDFs with an absolute box inside a box on every page render far faster

Rendering time grew with the square of the page count when an absolutely positioned box sat in a box broken across every page and was placed on the last page. A 2,000-row table that took 49 seconds now renders in 3 seconds, with the same PDF.
