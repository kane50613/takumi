---
packages:
  "takumi": patch
---

# Apply filters as Chrome does

- `blur()`, `backdrop-filter` and `text-shadow` blur with Chrome's Gaussian, at full resolution up to a 135px standard deviation.
- `drop-shadow()` reads its blur as the standard deviation, as CSS defines it. Image output used to halve it.
- A colored `drop-shadow()` fades out instead of painting an opaque halo.
- `drop-shadow()` rounds a fractional offset down, so `10.5px` lands where Chrome puts it.
- `grayscale()` and `sepia()` above 100% act as 100%.
- SVG keeps a fractional `hue-rotate()` angle.
