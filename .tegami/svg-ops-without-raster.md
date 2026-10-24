---
packages:
  takumi-core: minor
  takumi-pdf: patch
  takumi-svg: patch
---

# Draw an SVG's bitmaps and filtered groups without rasterizing it

`SvgOp::Raster` is gone. `SvgOp::Image` carries a bitmap inside an SVG and `SvgOp::PushFilter` marks a filtered group, and `SvgSource::vector_ops` no longer takes a raster scale. Resvg's raster renderer now sits behind the `svg-raster` feature, which is on by default. takumi-pdf embeds an SVG's PNG, JPEG and WebP images as PDF images and draws a filtered group without its filter. An `<img>` or `background-image` of an SVG under a CSS `filter` recolors its paints instead of rasterizing. takumi-svg writes an SVG's bitmaps as `<image>` elements.
