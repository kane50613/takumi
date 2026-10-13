---
packages:
  "takumi-core": patch
  "takumi": patch
---

# Upgrade taffy to 0.15

Layout runs on taffy 0.15, whose grid and flexbox passes are faster. The root box now sits at its own margins, as Chrome places it, instead of at the top left corner.
