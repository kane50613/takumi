---
packages:
  takumi-core:
    type: minor
  takumi-svg:
    type: patch
  takumi-pdf:
    type: patch
  takumi-raster:
    type: patch
---

### Skip the ink an underline runs through, in every backend

`text-decoration-skip-ink` reached only the raster backend, which read it off rasterized glyph coverage. The SVG and PDF backends drew straight through the glyphs.

An underline now breaks where the glyph outlines cross it, in all three. The break comes from the outline itself, so a gap inside a letter stays a gap instead of being swallowed with the strokes around it, and the width a break grows by follows the line's thickness the way a browser's does.
