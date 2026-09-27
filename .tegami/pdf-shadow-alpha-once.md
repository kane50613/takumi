---
packages:
  "takumi-pdf": patch
---

# Keep blurred translucent shadows at their own opacity in PDF output

A blurred `box-shadow` or `text-shadow` with a translucent color now applies that color's alpha once. Before, the stepped bands that fake the blur stacked it, so the shadow came out darker than its color.
