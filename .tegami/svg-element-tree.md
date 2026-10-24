---
packages:
  takumi-core: minor
---

# Parse SVG into a styled element tree

takumi-core adds an `svg` module that parses SVG markup into an element tree and computes each element's style the way Blink does. An SVG's intrinsic size now reads through it, so `svg-sizing` no longer depends on `svgtypes`.
