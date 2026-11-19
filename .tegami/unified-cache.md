---
packages:
  takumi-core: minor
  takumi-raster: minor
  takumi-svg: minor
  takumi-pdf: minor
  takumi: minor
---

# One `Cache` holds what renders reuse, with a budget for each kind of data

`Cache` replaces `ResourceCache` in the Rust crates. Pass one as `cache` to `RenderOptions`, `SvgOptions`, `PdfOptions`, `MeasureOptions`, or `PaintTreeOptions`, or leave it out to share `Cache::shared()`.

```rust
let cache = CacheOptions::builder()
  .images(64 << 20)
  .stylesheets(4 << 20)
  .build();

let options = RenderOptions::builder()
  .viewport(viewport)
  .node(node)
  .cache(cache.clone())
  .build();
```

Renders reuse more of what they decode:

- PDF renders reuse decoded images, where each call used to start empty.
- SVG and paint renders cache `data:` images, as raster renders do.
- A downscaled bitmap or SVG raster stays in the cache of the render that draws it.

#### Migration

`ResourceCache` and its `new`, `get_or_decode`, and `get_or_parse_stylesheet` methods are removed, and so is the `resource_cache` setter of `RenderOptions` and of the `takumi-pdf` options. Pass the cache as `cache`.

```diff
- let cache = ResourceCache::new(64 << 20);
- let image = cache.get_or_decode(&bytes, ImageCacheMode::Auto)?;
- let sheet = cache.get_or_parse_stylesheet(sources);
+ let cache = CacheOptions::builder().images(64 << 20).build();
+ let image = cache.decode_image(&bytes, ImageCacheMode::Auto)?;
+ let sheet = cache.parse_stylesheet(sources);
```
