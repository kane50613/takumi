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

`Renderer`, `PdfRenderer`, and `Painter` take `{ cache: { maxBytes, budgets } }`. `maxBytes`, 128 MiB by default, is one pool that decoded images, SVG pictures, stylesheets, Tailwind classes, glyphs, decoration intercepts, and `@font-face` faces share, and the renderer evicts what renders have not reused across all of them. `budgets` caps one kind, such as `{ images: 64 * 1024 * 1024 }`, and the bytes that kind holds come out of `maxBytes`. `budgets.fonts` caps the fonts that renders' `fonts` lists load, apart from `maxBytes`. `0` keeps nothing. In `takumi-js`, `setCacheOptions` sets the cache of the renderer `render` and the other functions share. `cacheMaxBytes`, which now sets `maxBytes`, `fontCacheMaxBytes`, `setGlyphCacheMaxBytes`, and the `FontCacheOptions` type still work, and are deprecated. `@takumi-rs/helpers` exports `CacheOptions`, `CacheBudgets`, and `RendererOptions`.
