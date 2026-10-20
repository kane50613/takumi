---
packages:
  takumi-svg: patch
---

# Draw SVG backgrounds and masks in SVG output with the render's fonts

A `background-image` or `mask-image` of an SVG `url(...)` is now written into SVG output as paths, like an `<img>` of an SVG. Its `<text>` uses the fonts the render registered and matches the PNG output. A tile too large to rasterize is skipped, as in PNG output.
