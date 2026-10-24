---
packages:
  takumi-core: minor
  takumi-raster: patch
---

# Draw SVG images in takumi-raster from the same ops as PDF

takumi-raster draws SVG images from `SvgOp`s with its new `SvgDrawing`, so takumi-core no longer carries resvg's renderer. `SvgSource::rasterize_cached` and `apply_svg_filter` take an `SvgDrawer`. `ImageSource::render_for_layout` drops its `current_color` and `fonts` arguments and returns `ImageError::SvgNotRasterized` for an SVG.
