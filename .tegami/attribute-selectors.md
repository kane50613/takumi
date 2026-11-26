---
packages:
  takumi-core: patch
  "@takumi-rs/helpers": patch
---

# `[dir]` and `img[src]` match elements parsed from HTML

Attribute selectors on `dir` and on an `<img>`'s `src` used to match only nodes built with JSX. `dir="auto"` in JSX also no longer fails the render.
