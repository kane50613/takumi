---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Paint overflow-clipping boxes in CSS paint order and let positioned descendants escape them

A box with `overflow: hidden` now paints its text above the backgrounds of later siblings, as Chrome does, instead of painting everything at once. A positioned box whose containing block sits outside such a box is no longer clipped by it, including under an `opacity` in between.
