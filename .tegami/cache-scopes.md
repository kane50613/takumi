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

A `Cache` now also keeps resolved glyphs and their masks, compiled Tailwind classes, the faces `@font-face` rules add, and decoration intercepts, each under a byte budget its builder sets: `Cache::builder().glyphs(32 << 20).build()`. These caches used to be global to the process, so two renderers shared them and only the glyph budget could be set. `set_glyph_cache_max_bytes` is gone from `takumi-core`. In JavaScript, `setGlyphCacheMaxBytes` sets the glyph budget of each `Renderer` created after the call, so call it before you create the renderer.
