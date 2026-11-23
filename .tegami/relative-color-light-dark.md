---
packages:
  takumi-core: patch
---

# Relative colours follow `color-scheme` through `light-dark()`

A relative colour whose origin uses `light-dark()`, such as `rgb(from light-dark(red, blue) r g b)`, takes the origin's colour in the element's used color scheme, as it does inside `color-mix()`.
