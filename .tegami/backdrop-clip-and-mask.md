---
packages:
  "takumi": patch
---

# Bound a backdrop filter by both clip-path and mask-image

An element with `backdrop-filter`, `clip-path`, and `mask-image` now shows its filtered backdrop only where both the clip and the mask let it through.
