---
packages:
  takumi-raster:
    type: patch
  takumi-svg:
    type: patch
  takumi:
    type: patch
---

### Place an inset `box-shadow` in the padding box

An inset `box-shadow` was placed against the border box instead of the padding box. On a box with a border the shadow fell at the border's position, where the opaque border covered it. The shadow is now placed against the padding box and appears just inside the border.
