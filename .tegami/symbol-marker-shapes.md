---
packages:
  takumi-core: patch
---

# Draw disc, circle, square, and disclosure list markers as shapes

These markers are now filled or stroked shapes sized from the marker's font, as Chrome draws them, in raster, SVG, PDF, and paint-tree output. They used to be text glyphs, so their size and position followed whichever font had the bullet character. An outside marker now sits as far from its item's text as it does in Chrome.
