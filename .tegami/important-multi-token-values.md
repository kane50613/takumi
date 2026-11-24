---
packages:
  takumi-core: patch
---

# Keep `!important` on shorthands and list values in stylesheets

A stylesheet or `style` attribute declaration such as `background: red !important`, `border-bottom: 1px solid red !important`, or `transform: scale(2) !important` now applies and wins over normal declarations, as in a browser. It used to be dropped.
