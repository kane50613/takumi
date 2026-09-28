---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Paint in CSS paint order, as Chrome does

- Positioned boxes and stacking contexts at `z-index: auto` paint above later in-flow siblings, and floats above in-flow block backgrounds, following CSS 2.1 Appendix E. A `position: relative` box nudged over the next block no longer disappears under it.
