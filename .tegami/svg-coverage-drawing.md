---
packages:
  takumi-core: minor
  takumi-raster: patch
---

# SVG images shade edges and gradients closer to Chromium

SVG images in raster output shade a little differently. A shape edge that falls between pixels can shade a pixel 1 to 2 levels differently from before, a gradient with a hard stop at its start paints the first stop's colour before it, and a `pixelated` image whose texel edges land on pixel centers takes the texel before the edge, as Chromium does.

A `rect` without radii now fills as a rect, as in Chromium, so a rect stretched to a fractional size keeps a solid edge.

#### Migration

`SvgOp::Draw` has a new `rect` field. Set it to `true` only when `path` outlines a `rect` without radii, which then fills as a rect. Set it to `false` for any other path.

```diff
  SvgOp::Draw {
    path,
    fill,
    stroke,
    anti_alias,
+   rect: false,
  }
```
