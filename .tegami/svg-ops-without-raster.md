---
packages:
  takumi-core: minor
  takumi-pdf: patch
  takumi-svg: patch
---

# Draw an SVG's bitmaps and filtered groups in a PDF without rasterizing

takumi-pdf embeds the PNG, JPEG, and WebP images inside an SVG as PDF images, and draws an SVG group without its filter. A CSS color `filter` on an SVG `<img>` or `background-image` recolors the SVG instead of rasterizing it, and takumi-svg writes an SVG's bitmaps as `<image>` elements. `SvgOp::Image` and `SvgOp::PushFilter` replace `SvgOp::Raster`, `SvgSource::vector_ops` no longer takes a raster scale, and resvg's renderer moves behind the `svg-raster` feature, on by default.
