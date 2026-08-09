---
packages:
  takumi-core:
    type: patch
  takumi-pdf:
    type: patch
---

### Paint text decorations from one place

`paint_run_decorations` fills the underline, overline, and line-through of a glyph run, so a backend supplies a device rather than its own rect fill.

A `PaintDevice` now takes a transform instead of an origin, since a decoration under rotated text needs the whole matrix.
