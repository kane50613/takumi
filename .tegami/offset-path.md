---
packages:
  "cargo:takumi-css": minor
---

### Add CSS Motion Path support

Elements (and text, glyph by glyph) can ride an arbitrary `offset-path`. The
path accepts `ray()`, the basic shapes `path()`/`circle()`/`ellipse()`/`polygon()`/`inset()`,
and a bare `<coord-box>`; `offset-distance`, `offset-rotate`, `offset-anchor`,
`offset-position`, and the `offset` shorthand control placement. Animate
`offset-distance` with `@keyframes` to move along the path. `url()` references
are not supported.
