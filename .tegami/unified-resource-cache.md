---
packages:
  "cargo:takumi-core": minor
  "cargo:takumi-raster": minor
  "cargo:takumi-svg": minor
  "cargo:takumi-bindings-common": minor
  "npm:@takumi-rs/core": minor
  "npm:@takumi-rs/wasm": minor
---

### Unify decoded resources behind one budgeted cache

Memory retention was governed by differently-shaped caches: decoded images had a byte budget, but each SVG kept up to 32 rasterized pixmaps outside it, and every render re-parsed its stylesheets from scratch. `ImageCache` is now `ResourceCache`: SVG sources, their rasterized pixmaps, and parsed stylesheets all weigh against the same budget as decoded images. The default budget drops from 64 MiB to 16 MiB and becomes configurable — `new Renderer({ cacheMaxBytes })` in the bindings, `ResourceCache::new(max_bytes)` in Rust, with `0` disabling caching. SVG rasters and parsed stylesheets now also survive across renders, so a server re-rendering the same template stops re-rasterizing and re-parsing per request. Rust callers: `RenderOptions.stylesheet` is now `Arc<StyleSheet>`; pass `sheet.into()`.
