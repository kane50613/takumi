---
packages:
  "takumi": patch
---

# Clip images to their content box's curve

A rounded image now clips to the curve of its content box, the border radius less the border and padding, as Chrome clips it. The image output used to round the corners of the image itself, so an image smaller than its box under `object-fit: contain` came out rounded, and the SVG output clipped to the padding box instead.
