---
packages:
  "takumi": patch
---

# Keep very large blurs as wide as Chrome's

A blur with a standard deviation above 135px, such as `filter: blur(160px)`, now shrinks the image, blurs it and scales it back up as Chrome does. Before, the blur stopped widening at that size.
