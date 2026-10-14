---
packages:
  "takumi-core": patch
  "takumi": patch
---

# Move inline elements by `position: relative`

An inline element with `position: relative` now moves by its `top`, `left`, `bottom` and `right`, along with its background, border, text decorations and inline-block children, as in Chrome. Tailwind's Preflight relies on this to raise `<sup>` and lower `<sub>`.
