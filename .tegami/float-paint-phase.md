---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Paint floats below the text they overlap

A float inside text now paints before that text, as Chrome paints floats in their own phase, so text running over a float stays on top of it.
