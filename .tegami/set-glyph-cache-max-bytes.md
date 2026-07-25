---
packages:
  "@takumi-rs/core": minor
  "@takumi-rs/wasm": minor
  "takumi-js": minor
---

### Add `setGlyphCacheMaxBytes`

The resolved-glyph and glyph-mask caches share an 8 MiB budget that no binding exposed. `cacheMaxBytes` looks like the knob for it but covers a different set of caches: decoded images, SVG rasters, and parsed stylesheets.

`setGlyphCacheMaxBytes` sets the glyph budget. It is a module-level function rather than a `Renderer` option because those caches live in the module and are shared by every renderer, and the budget is read the first time a cache is used, so the call has to come before the first render.

The default suits Latin text. A CJK outline runs a few kilobytes, so 8 MiB holds on the order of a thousand of them and a page of Chinese re-rasterizes glyphs it evicted a moment earlier.

`takumi-js` forwards it too. That one is async, since it has to resolve the backend before it can set anything.
