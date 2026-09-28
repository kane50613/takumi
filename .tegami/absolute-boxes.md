---
packages:
  "takumi": patch
---

# Place absolutely positioned boxes as Chrome does

- An `auto`-width absolute box wraps its content to the containing block's width minus its insets and margins, so `left: 50%` text no longer runs past the right edge.
