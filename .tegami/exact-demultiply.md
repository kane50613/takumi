---
packages:
  takumi-core: patch
---

# Gradient stops in SVG output round exactly

A translucent gradient stop that `takumi-svg` and `takumi-pdf` write out now converts back from premultiplied alpha with exact rounding. A channel that falls on an exact half now rounds up, so a stop that came out as `#007fff` is now `#0080ff`.
