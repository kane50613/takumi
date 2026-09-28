---
packages:
  "takumi": patch
---

# Apply filters as Chrome does

- `drop-shadow()` reads its blur length as the Gaussian's standard deviation. The image output used to halve it like a `box-shadow` blur.
- `grayscale()` and `sepia()` above 100% act as 100% in the image output, and round each channel to the nearest value.
- A fractional `hue-rotate()` angle no longer rounds to whole degrees in the SVG output.
