---
packages:
  "takumi": patch
---

# Snap fractional drop-shadow offsets down, as Chrome does

`filter: drop-shadow()` now moves its shadow by the offset rounded down to whole pixels in the image and SVG output, where it used to round to nearest, so `10.5px` lands where Chrome puts it.
