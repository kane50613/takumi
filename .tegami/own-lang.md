---
packages:
  "cargo:takumi-raster": minor
  "cargo:takumi-svg": minor
---

### Take a BCP-47 string for `lang` instead of `parley::Language`

The render and SVG builders no longer require constructing a `parley::Language`; they accept an owned BCP-47 tag and parse it internally.
