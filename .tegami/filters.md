---
packages:
  "takumi": patch
---

# Apply filters as Chrome does

- `drop-shadow()` reads its blur length as the Gaussian's standard deviation. The image output used to halve it like a `box-shadow` blur.
- `grayscale()` and `sepia()` above 100% act as 100% in the image output, and round each channel to the nearest value.
- A fractional `hue-rotate()` angle no longer rounds to whole degrees in the SVG output.
- `drop-shadow()` rounds a fractional offset down to whole pixels in the image and SVG output, so `10.5px` lands where Chrome puts it.
- `blur()`, `backdrop-filter` and `text-shadow` blur with the Gaussian Chrome's software renderer uses: a true kernel for small blurs and three box passes for larger ones. A blur keeps full resolution up to a standard deviation of 135px, then shrinks, blurs and scales back up as Chrome does.
- A colored `drop-shadow()` fades out as Chrome's does. It used to paint an opaque halo of its color, and an element's anti-aliased edge came out lighter than the shadow behind it.
