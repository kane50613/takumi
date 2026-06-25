---
packages:
  "cargo:takumi-css": patch
  "cargo:takumi-raster": patch
  "cargo:takumi-svg": patch
---

### Fix `path()` ignoring device-pixel-ratio

`offset-path: path()` and `clip-path: path()` used the authored SVG coordinates verbatim while every other basic shape crossed the dpr boundary through `to_px`. At `devicePixelRatio != 1` the path was mis-scaled by `1/dpr`. The coordinates are now scaled into device space like the other shapes.
