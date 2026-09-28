---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Clip as Chrome does

- A percentage corner radius in `clip-path: inset(... round ...)` resolves its vertical radius against the box height in the SVG output.
- A `path()` clip that cannot be parsed no longer hides its element in the image output.
- An element with more than one of `clip-path`, `mask-image` and a clipping `overflow` applies all of them in the image output.
- A rounded image clips to the curve of its content box, the border radius less the border and padding.
- `circle()` in `clip-path` and `offset-path` resolves a percentage radius against the box's diagonal over √2, and `closest-side` or `farthest-side` against all four sides.
