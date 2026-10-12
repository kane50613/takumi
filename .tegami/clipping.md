---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Clip as Chrome does

- An element with more than one of `clip-path`, `mask-image` and a clipping `overflow` applies all of them.
- A `path()` clip that fails to parse no longer hides the element.
- A rounded image clips to the curve of its content box.
- In `circle()`, a percentage radius and `closest-side` or `farthest-side` resolve as CSS defines them.
- SVG resolves a percentage vertical radius in `inset(... round ...)` against the box height.
