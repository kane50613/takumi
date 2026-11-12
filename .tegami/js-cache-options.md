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

# Every renderer takes its cache budgets in one `cache` option

`Renderer`, `PdfRenderer`, and `Painter` take `{ cache: { images, pictures, stylesheets, tailwind, glyphs, intercepts, fontFaces, fonts } }`, each a byte budget that keeps its default when left out. `PdfRenderer` and `Painter` could only set the font budget before, and the glyph budget needed `setGlyphCacheMaxBytes`. In `takumi-js`, `setCacheOptions` sets the budgets of the renderer `render` and the other functions share. `cacheMaxBytes`, `fontCacheMaxBytes`, `setGlyphCacheMaxBytes`, and the `FontCacheOptions` type still work, and are deprecated. `@takumi-rs/helpers` exports `CacheOptions` and `RendererOptions`, which every binding re-exports.
