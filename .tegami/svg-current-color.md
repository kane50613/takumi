---
packages:
  takumi-core:
    type: minor
  takumi-raster:
    type: patch
  takumi-pdf:
    type: patch
  takumi-svg:
    type: patch
---

### Resolve SVG `currentColor` against the host `color`

`currentColor` inside an SVG image now inherits the host element's `color`. The lookup matches an inline `<svg>` in a browser: a `color` attribute inside the SVG wins, then the host color, then black. `render_for_layout` and `vector_ops` take the current color as an argument.
