---
packages:
  takumi-core:
    type: minor
  "@takumi-rs/core":
    type: patch
  "@takumi-rs/wasm":
    type: patch
---

# Glyphs, Tailwind classes, and `@font-face` faces stay in the render's `Cache`

Two renderers in one process no longer share glyphs, compiled Tailwind classes, `@font-face` faces, or decoration intercepts. Each `Cache` keeps its own glyphs in the pool it shares with every other kind of data. `glyphs(ImageCacheMode::None)` keeps none.

```rust
let cache = CacheOptions::builder().glyphs(ImageCacheMode::None).build();
```

In `@takumi-rs/core` and `@takumi-rs/wasm`, `setGlyphCacheMaxBytes(0)` now keeps no glyphs only in renderers created after the call, and any other size keeps them in the shared pool. In `takumi-js`, it reaches only the renderer behind `render`, not a `Renderer` from `takumi-js/node`.

#### Migration

`set_glyph_cache_max_bytes` in `takumi-core` is deprecated. A size of `0` still keeps no glyphs in the caches built after the call, and a larger size no longer bounds them apart.

```diff
- set_glyph_cache_max_bytes(0);
- let cache = CacheOptions::builder().build();
+ let cache = CacheOptions::builder().glyphs(ImageCacheMode::None).build();
```
