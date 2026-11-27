---
packages:
  takumi-core: minor
  takumi-raster: minor
---

# Rasterizing an SVG image through `takumi-core` takes an `SvgRasterizer`

Code that turns an SVG `ImageSource` into pixels through `takumi-core` now passes an `SvgRasterizer`, such as `takumi_raster::SvgPixmapRasterizer`. Rendering through `takumi-raster` needs no change. A backend that runs SVG filters itself reads them from `takumi_core::svg_filter`.

- `SvgSource::rasterize_cached` takes the rasterizer that draws a picture the cache does not hold yet.
- `ImageSource::render_for_layout` returns `ImageError::SvgParseNotSupported` for an SVG.
- `apply_svg_filter` is gone from `takumi_core::resources::image`. `takumi-raster` applies a referenced SVG filter itself.

#### Migration

```diff
- let rendered = image.render_for_layout(width, height, algorithm, 0, &host)?;
+ if let ImageSource::Svg(svg) = &image {
+   let buffer = svg.rasterize_cached(
+     SvgRaster::whole(viewport, width, height),
+     algorithm,
+     &host,
+     &takumi_raster::SvgPixmapRasterizer,
+   )?;
+ } else {
+   let rendered = image.render_for_layout(width, height, algorithm, 0, &host)?;
+ }
```
