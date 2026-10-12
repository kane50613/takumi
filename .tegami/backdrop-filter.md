---
packages:
  "takumi": patch
---

# Apply `backdrop-filter` as Chrome does

- The filtered backdrop draws over the original, so `opacity()` no longer punches a hole.
- With both `clip-path` and `mask-image`, the backdrop shows only where both let it through.
- A blurred backdrop reads only what is inside the element, so content just outside no longer bleeds in.
