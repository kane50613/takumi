---
packages:
  "takumi": patch
---

# Composite a filtered backdrop over the original, as Chrome does

`backdrop-filter` now lays the filtered backdrop over what it came from instead of replacing it, so `opacity()` no longer punches a translucent hole and the image output matches the SVG output and Chrome.
