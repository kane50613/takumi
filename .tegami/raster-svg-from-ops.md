---
packages:
  takumi-core: minor
  takumi-raster: patch
---

# Rasterize SVG images from the same ops as PDF

takumi-core now rasterizes an SVG from the `SvgOp` list with tiny-skia, so resvg's renderer is gone and the raster, PDF, and SVG backends draw one flattened tree. `SvgOp` gains anti-aliasing, the radial focal radius, and a `miter-clip` join, which resvg drew and the ops had dropped. `BlendMode::to_tiny_skia` replaces takumi-raster's private conversion.
