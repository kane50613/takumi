---
packages:
  "cargo:takumi-css": minor
  "cargo:takumi": minor
---

### Add CSS `offset-path` support

The path accepts `ray()`, the basic shapes `path()`/`circle()`/`ellipse()`/`polygon()`/`inset()`,
and a bare `<coord-box>`

`offset-distance`, `offset-rotate`, `offset-anchor`, `offset-position`, and the `offset` shorthand control placement.

`url()` references are not supported.
