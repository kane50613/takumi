---
packages:
  "cargo:takumi-core": patch
---

### Include glyph ink extents in text paint bounds

Text paint bounds only covered the advance × (ascent + descent) metrics box, so isolation surfaces (opacity, filters) clipped ink outside it: synthetic-italic overhang, faux-bold outset, and negative bearings. Node paint bounds now merge each glyph's ink extents.
