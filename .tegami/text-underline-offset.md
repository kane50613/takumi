---
packages:
  "cargo:takumi-css": patch
  "cargo:takumi-core": patch
---

### Support `text-underline-offset`

Add the `text-underline-offset` property, accepting `auto` or a `<length-percentage>` that shifts the underline away from the text. Percentages resolve against `1em`. Applies to the raster and SVG backends.
