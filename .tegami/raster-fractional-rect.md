---
packages:
  "takumi": patch
---

# Antialias rectangles that end between pixels in raster output

A solid rectangle whose edge or size falls between pixels, such as a decoration line or an inline box, now covers its edge pixels in proportion, as the SVG output does. Before, raster output cut the width down to a whole pixel.
