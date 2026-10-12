---
packages:
  "takumi": patch
---

# Start a stacking context for any transform and `position: fixed`

Any transform property, even `translate: 0 0`, and `position: fixed` start a stacking context, as in Chrome. The box paints above later in-flow boxes and keeps its `z-index: -1` children above its background.
