---
packages:
  "takumi-core": patch
  "takumi": patch
---

# Keep no fragment items for hidden subtrees

Laying out a `display: none` box no longer keeps fragment items for the boxes under it, which paint never reads. A hidden 48-card panel costs 0.3 ms instead of 2.1 ms per render.
