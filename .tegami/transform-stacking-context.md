---
packages:
  "takumi": patch
---

# Make an identity transform and `position: fixed` start a stacking context

A box with any transform-related property, even one that moves nothing, such as `transform: translateX(0)` or `translate: 0 0`, now starts a stacking context, as in Chrome. So does a `position: fixed` box. It now paints above the in-flow boxes after it and keeps its `z-index: -1` children above its own background.
