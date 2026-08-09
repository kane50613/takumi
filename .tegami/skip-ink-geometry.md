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

`text-decoration-skip-ink` breaks an underline where the glyph outlines cross it, in every backend. A gap inside a letter stays a gap.
