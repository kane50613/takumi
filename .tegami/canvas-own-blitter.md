---
packages:
  takumi-raster: patch
---

# Rotated and skewed boxes draw with antialiased edges

A rotated or skewed image, gradient, or color box now draws with antialiased edges, as in Chromium. Previously, some rotated or skewed boxes had jagged edges.

A `background-clip: text` background on a box at a fractional position now lines up with the device pixel grid, so the top row of its text no longer fades. A translucent `plus-lighter` layer now adds its colours as Chromium does, where it used to come out too dark.
