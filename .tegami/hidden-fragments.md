---
packages:
  "takumi-core": patch
  "takumi": patch
---

# Skip text layout in hidden subtrees

Text under a `display: none` box is no longer shaped and laid out. A hidden 48-card panel costs 0.3 ms instead of 2.1 ms per render.
