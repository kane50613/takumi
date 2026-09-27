---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Paint positioned boxes and floats in CSS paint order

Positioned boxes and stacking contexts at `z-index: auto` now paint above later in-flow siblings, and block-level floats above in-flow block backgrounds, following CSS 2.1 Appendix E as browsers do. A `position: relative` box nudged over the next block no longer disappears under it.
