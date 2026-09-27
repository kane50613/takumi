---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Clip `border-area` backgrounds to dashed, dotted and double borders

`background-clip: border-area` now keeps the background only where a dashed, dotted or double border actually paints, as Chrome masks it. Before, it filled the whole border ring.
