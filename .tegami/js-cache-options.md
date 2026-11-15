---
packages:
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

# Every renderer takes its cache size in one `cache` option

`Renderer`, `PdfRenderer`, and `Painter` take `{ cache: { maxBytes, budgets } }`. `maxBytes`, 128 MiB by default, is one pool for the images, SVG pictures, stylesheets, glyphs, and other data a renderer reuses, and `budgets` caps one kind, such as `{ images: 64 * 1024 * 1024 }`. In `takumi-js`, `setCacheOptions` sets the cache of the renderer the top-level functions share. `cacheMaxBytes`, `fontCacheMaxBytes`, `setGlyphCacheMaxBytes`, and `FontCacheOptions` still work, and are deprecated. `@takumi-rs/helpers` exports the option types.
