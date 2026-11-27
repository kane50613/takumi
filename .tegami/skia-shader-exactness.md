---
packages:
  takumi-raster: patch
---

# Transformed images and gradients sample as in Chromium, and filter regions keep their edge pixels

Images and gradients under a transform now sample where Chromium samples them, and SVG filters keep the pixels at the fractional edge of their regions.

- An image or gradient scaled down so far that its transform's determinant falls below about `1.2e-7` now draws, as in Chromium. A gradient in an SVG with a `viewBox` 100,000 units wide drawn at 20 px is one such case.
- Their transforms are inverted in double precision, as Chromium does, so a few sampled pixels can shade a level differently.
- A filter region or primitive subregion with a fractional edge now includes the pixel that edge cuts, as Chromium's crop does.
