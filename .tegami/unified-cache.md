---
packages:
  takumi-core: minor
  takumi-raster: minor
  takumi-svg: minor
  takumi-pdf: minor
  takumi: minor
---

# One `Cache` holds what renders reuse, in one pool of bytes

`Cache` replaces `ResourceCache` in the Rust crates. Every kind of data it keeps shares `max_bytes`, 128 MiB by default, and the cache evicts what renders have not reused, whatever its kind. A setter named after one kind, such as `images`, caps that kind, and the bytes it holds come out of `max_bytes`.

```rust
let cache = Cache::builder()
  .max_bytes(256 << 20)
  .images(64 << 20)
  .build();

let options = RenderOptions::builder()
  .viewport(viewport)
  .node(node)
  .cache(cache.clone())
  .build();
```

Pass a `Cache` as `cache` to `RenderOptions`, `SvgOptions`, `PdfOptions`, `MeasureOptions`, or `PaintTreeOptions`. A render without one shares `Cache::shared()`, and renders reuse more of what they decode:

- PDF renders reuse decoded images, where each call used to start empty.
- SVG and paint renders cache `data:` images, as raster renders do.
- A downscaled bitmap or SVG raster stays in the cache of the render that draws it.

#### Migration

`ResourceCache` and its `new`, `get_or_decode`, and `get_or_parse_stylesheet` methods are removed, and so is the `resource_cache` setter of the `takumi-pdf` options. The `resource_cache` setter of `RenderOptions` still works.

```diff
- let cache = ResourceCache::new(256 << 20);
- let image = cache.get_or_decode(&bytes, ImageCacheMode::Auto)?;
- let sheet = cache.get_or_parse_stylesheet(sources);
+ let cache = Cache::builder().max_bytes(256 << 20).build();
+ let image = cache.decode_image(&bytes, ImageCacheMode::Auto)?;
+ let sheet = cache.parse_stylesheet(sources);
```
