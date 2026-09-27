---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Paint a repeating gradient with coincident stops in its last colour

A repeating gradient whose stops all sit at one position now paints solid in the last stop's colour, as Chrome does, instead of splitting into two colours.
