---
packages:
  takumi-core: patch
---

# Whole percentages resolve exactly

A whole-number percentage such as `30%` now parses as exactly 30, as in Chromium, where it used to come out a hair above. A dashed SVG stroke with percentage lengths now matches the same stroke written in pixels, and an SVG filter region of `60%` no longer leaves a faint line past its edge.
