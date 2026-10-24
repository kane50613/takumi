---
packages:
  takumi-core: minor
---

# Lay out SVG from takumi's own element tree

takumi-core builds the SVG render tree from its `svg` module instead of usvg's parser, and `svgtypes` is gone. An invalid style declaration is dropped as in Blink, `font-size` keywords are absolute, and `plus-lighter`, `font-weight` numbers, and empty filter functions such as `grayscale()` follow CSS.
