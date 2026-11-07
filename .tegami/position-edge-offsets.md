---
packages:
  takumi-core: patch
  takumi-raster: patch
---

# Positions take offsets from a side, and an offset image keeps its pixels

`object-position: bottom 1px right 2px` used to make its declaration invalid, so the image stayed centered. A position now takes the four-value form, where each side keyword takes an offset from that side, and `background-position` and `mask-position` also take the three-value form, as in Chromium.

An image drawn smaller than its file and reaching past its content box, such as one an offset moves, used to be squeezed into the part that shows. It now draws at its full size, and the box crops it.
