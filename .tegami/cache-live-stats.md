---
packages:
  takumi-core:
    type: minor
  takumi:
    type: minor
  "@takumi-rs/helpers":
    type: minor
  "@takumi-rs/core":
    type: minor
  "@takumi-rs/wasm":
    type: minor
  takumi-pdf:
    type: minor
  takumi-paint:
    type: minor
  takumi-js:
    type: minor
---

# Change a renderer's cache while it runs, and see what the cache holds

`Renderer`, `PdfRenderer`, and `Painter` have `setCacheOptions(options)`, which takes the constructor's `cache` options and applies them to the renderer in use. A smaller `maxBytes` or budget evicts what no longer fits right away, and `budgets.fonts` releases fonts the same way. `cacheStats()` reports the bytes the pool holds now, its high-water mark, and its `maxBytes` under `memory`, and each kind's bytes, budget, hits, misses, and evictions under its `budgets` key. In `takumi-js`, `setCacheOptions` now applies to the shared renderer after the first render too, where it used to be ignored, and `cacheStats()` reports that renderer's cache. In Rust, `Cache::set_options` applies a `CacheOptions` to a cache in use, and `Cache::stats` returns a `CacheStats`. `CacheOptions` fields are public, and `CacheOptions::default()` starts from the default size. `@takumi-rs/helpers` exports `CacheStats`, `CacheMemoryStats`, and `CacheKindStats`.
