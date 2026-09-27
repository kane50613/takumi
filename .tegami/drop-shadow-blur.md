---
packages:
  "takumi": patch
---

# Blur `drop-shadow()` as much as browsers do

The blur length of `filter: drop-shadow()` is the Gaussian's standard deviation, where a `box-shadow` blur is twice that. The image output halved it like a box shadow, so its drop shadows came out half as soft as in Chrome and in the SVG output. A fractional `hue-rotate()` angle also no longer rounds to whole degrees in the SVG output.
