---
packages:
  takumi-core: minor
  takumi-raster: patch
---

# Rasterize SVG images from their ops in the raster backend

takumi-raster draws an SVG image from the same `SvgOp` list the PDF and SVG backends use, through the new `SvgDrawing`, and no longer through resvg's renderer. `SvgSource::rasterize_cached` and `apply_svg_filter` take an `SvgDrawer`, and `ImageSource::render_for_layout` drops its `current_color` and `fonts` arguments and returns `ImageError::SvgNotRasterized` for an SVG. Edges of SVG shapes can differ from before by antialiasing.
