---
packages:
  "takumi-core": patch
  "takumi": patch
---

# Share the layout style of anonymous boxes

Anonymous block boxes share one layout style instead of each holding a copy, so text-heavy tables render in less memory.
