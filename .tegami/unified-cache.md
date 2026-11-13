---
packages:
  takumi-core: minor
  takumi-raster: minor
  takumi-svg: minor
  takumi-pdf: minor
  takumi: minor
---

# One `Cache` holds what renders reuse, with a builder for each budget

`Cache` replaces `ResourceCache`. Build one with `Cache::builder().images(64 << 20).stylesheets(4 << 20).build()` and pass it as `cache` to `RenderOptions`, `SvgOptions`, `PdfOptions`, `MeasureOptions`, or `PaintTreeOptions`. A render without one shares `Cache::shared()`, so PDF renders reuse decoded images where each call used to start empty. `get_or_decode` and `get_or_parse_stylesheet` are now `decode_image` and `parse_stylesheet`. Code written for `ResourceCache`, the old method names, and the `resource_cache` builder setters still builds, with `ResourceCache` and the old methods deprecated. A downscaled bitmap or SVG raster stays in the cache of the render that draws it, and SVG and paint renders now cache `data:` images as raster renders do.
