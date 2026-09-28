---
packages:
  "takumi": patch
---

# Apply `backdrop-filter` as Chrome does

- The filtered backdrop composites over the original instead of replacing it, so `opacity()` no longer punches a translucent hole.
- An element with both `clip-path` and `mask-image` shows its filtered backdrop only where both let it through.
