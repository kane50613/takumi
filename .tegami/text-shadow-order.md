---
packages:
  takumi-core:
    type: minor
  takumi-raster:
    type: patch
---

### Stack text shadows the way a browser does

The raster backend painted `text-shadow` in list order, putting the last one on top. CSS puts the first one there, so a stack of glows came out inverted. `SizedFontStyle::painted_text_shadows` walks them back to front for every backend, and drops the ones nobody sees.
