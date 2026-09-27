---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Paint visible children of `visibility: hidden` elements

A `visibility: hidden` element now hides only its own box, text, image, and outline, so descendants that set `visibility: visible` paint, as browsers show them. `opacity: 0` and `display: none` still hide the whole subtree.
