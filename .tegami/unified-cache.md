---
packages:
  takumi-core: minor
  takumi-raster: minor
  takumi-svg: minor
  takumi-pdf: minor
  takumi: minor
---

# One `Cache` holds what renders reuse, in one pool of bytes

`Cache` replaces `ResourceCache`. Build one with `Cache::builder().max_bytes(256 << 20).build()` and pass it as `cache` to `RenderOptions`, `SvgOptions`, `PdfOptions`, `MeasureOptions`, or `PaintTreeOptions`. A render without one shares `Cache::shared()`, so PDF renders reuse decoded images where each call used to start empty. Every kind of data the cache keeps shares `max_bytes`, 128 MiB by default, and the cache evicts what renders have not reused across all of them. A setter named after one kind, such as `images(64 << 20)`, caps that kind, and the bytes it holds come out of `max_bytes`. `ResourceCache::new(max_bytes)` sets `max_bytes`. `get_or_decode` and `get_or_parse_stylesheet` are now `decode_image` and `parse_stylesheet`. Code written for `ResourceCache`, the old method names, and the `resource_cache` builder setters still builds, with `ResourceCache` and the old methods deprecated. A downscaled bitmap or SVG raster stays in the cache of the render that draws it, and SVG and paint renders now cache `data:` images as raster renders do.
