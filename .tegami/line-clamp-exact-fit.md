---
packages:
  "takumi-core": patch
---

# Keep every line `line-clamp` allows

`line-clamp` now limits text by line count alone, as Chrome does. A paragraph that fits its clamp exactly no longer loses its last line to rounding, such as two lines of 42px text at `line-height: 1.04`.
