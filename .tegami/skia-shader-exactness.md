---
packages:
  takumi-raster: patch
---

# Transformed images and gradients sample as in Chromium

Images and gradients under a transform now invert it in double precision with Chromium's cutoff, so a few sampled pixels can shade a level differently. One scaled down so far that its determinant falls below about `1.2e-7` now draws, such as a gradient in an SVG whose `viewBox` is 100,000 units wide drawn at 20 px. An SVG filter region or primitive subregion with a fractional edge now keeps the pixel that edge cuts, as Chromium's crop does.
