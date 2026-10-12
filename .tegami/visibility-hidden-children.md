---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Paint visible children of `visibility: hidden` elements

A `visibility: hidden` element hides only itself, so children with `visibility: visible` paint. `opacity: 0` and `display: none` still hide the whole subtree.
