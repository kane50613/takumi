---
packages:
  takumi-core: patch
---

# Lay an SVG in an SVG `<image>` out at the image's size

An SVG that an SVG `<image>` or `feImage` shows now lays out at the image's rectangle, as in Chrome, so its viewport units and percentages measure that rectangle.
