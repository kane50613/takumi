---
packages:
  "takumi": patch
---

# Composite clipped layers faster

A layer from `opacity`, `filter` or a blend mode inside a clip, such as `overflow: hidden` or `border-radius`, is cut to the clip a run of pixels at a time. Pages with large blurred or translucent backgrounds render up to 25% faster.
