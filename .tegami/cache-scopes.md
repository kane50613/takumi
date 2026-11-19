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

Two renderers in one process no longer share glyphs, compiled Tailwind classes, `@font-face` faces, or decoration intercepts. Each `Cache` now keeps its own. Before, these caches were global to the process. The builder sets the glyph budget, as `set_glyph_cache_max_bytes` did for the whole process.

```rust
let cache = CacheOptions::builder().glyphs(32 << 20).build();
```

In JavaScript, `setGlyphCacheMaxBytes` sets the glyph budget of each `Renderer` created after the call, so call it before you create the renderer.

#### Migration

`set_glyph_cache_max_bytes` in `takumi-core` is deprecated. It still sets the glyph budget of the caches built after the call that set none of their own.

```diff
- set_glyph_cache_max_bytes(32 << 20);
- let cache = CacheOptions::builder().build();
+ let cache = CacheOptions::builder().glyphs(32 << 20).build();
```
