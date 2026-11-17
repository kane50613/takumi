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

Two renderers in one process no longer share glyphs, compiled Tailwind classes, `@font-face` faces, or decoration intercepts. Each `Cache` now keeps its own, under a byte budget its builder sets. Before, these caches were global to the process, and only the glyph budget could be set.

```rust
let cache = Cache::builder().glyphs(32 << 20).build();
```

The builder has a setter for each of them:

- `glyphs` for resolved glyphs and their masks
- `tailwind` for compiled Tailwind classes
- `font_faces` for the faces `@font-face` rules add
- `intercepts` for where glyphs cross a decoration line, which `text-decoration-skip-ink` reads

In JavaScript, `setGlyphCacheMaxBytes` sets the glyph budget of each `Renderer` created after the call, so call it before you create the renderer.

#### Migration

`set_glyph_cache_max_bytes` in `takumi-core` is deprecated. It still sets the glyph budget of the caches built after the call that set none of their own.

```diff
- set_glyph_cache_max_bytes(32 << 20);
- let cache = Cache::builder().build();
+ let cache = Cache::builder().glyphs(32 << 20).build();
```
