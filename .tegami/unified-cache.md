---
packages:
  takumi-core: minor
  takumi-raster: minor
  takumi-svg: minor
  takumi-pdf: minor
  takumi: minor
---

# One `Cache` holds what renders reuse, in one pool of bytes

`Cache` replaces `ResourceCache` in the Rust crates. Everything it keeps shares `max_bytes`, 128 MiB by default. Setting `images` or `glyphs` to `ImageCacheMode::None` keeps none of that kind.

```rust
let cache = CacheOptions::builder()
  .max_bytes(256 << 20)
  .images(ImageCacheMode::None)
  .build();

let options = RenderOptions::builder()
  .viewport(viewport)
  .node(node)
  .cache(cache.clone())
  .build();
```

`RenderOptions`, `SvgOptions`, `PdfOptions`, `MeasureOptions`, and `PaintTreeOptions` take it as `cache`. A render without one shares `Cache::shared()`. PDF renders now reuse decoded images, and SVG and paint renders cache `data:` images.

#### Migration

`ResourceCache` and the `resource_cache` setter of `RenderOptions` and of the `takumi-pdf` options are removed. Pass the cache as `cache`.

```diff
- let cache = ResourceCache::new(256 << 20);
- let image = cache.get_or_decode(&bytes, ImageCacheMode::Auto)?;
- let sheet = cache.get_or_parse_stylesheet(sources);
+ let cache = CacheOptions::builder().max_bytes(256 << 20).build();
+ let image = cache.decode_image(&bytes, ImageCacheMode::Auto)?;
+ let sheet = cache.parse_stylesheet(sources);
```
