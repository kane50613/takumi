---
packages:
  takumi-pdf:
    type: minor
---

### Rasterize `filter: blur()` and `drop-shadow()`

The two convolution filters now render. The filtered subtree rasterizes through takumi-raster the way Chromium renders a filtered layer offscreen, and embeds as an image; every filter function in the list applies during that pass, in order. Elements with only color filters keep the vector path.

The raster pass ships behind the `raster-filters` feature, on by default, and grows the wasm binary by about 8%.
