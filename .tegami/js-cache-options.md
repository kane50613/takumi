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

`Renderer`, `PdfRenderer`, and `Painter` take a `cache` option. Its `maxBytes`, 128 MiB by default, is one pool for the images, SVG pictures, stylesheets, glyphs, and other data a renderer reuses. Its `budgets` cap one kind on its own.

```ts
const renderer = new Renderer({
  cache: {
    maxBytes: 256 * 1024 * 1024,
    budgets: { images: 64 * 1024 * 1024 },
  },
});
```

In `takumi-js`, `setCacheOptions` sets the cache of the renderer that `render` and the other functions share. `@takumi-rs/helpers` exports the option types.

#### Migration

`cacheMaxBytes`, `fontCacheMaxBytes`, `setGlyphCacheMaxBytes`, and `FontCacheOptions` still work, and are deprecated.

```diff
  // @takumi-rs/core
- setGlyphCacheMaxBytes(16 * 1024 * 1024);
- const renderer = new Renderer({ cacheMaxBytes: 64 * 1024 * 1024, fontCacheMaxBytes: 32 * 1024 * 1024 });
+ const renderer = new Renderer({
+   cache: {
+     maxBytes: 64 * 1024 * 1024,
+     budgets: { glyphs: 16 * 1024 * 1024, fonts: 32 * 1024 * 1024 },
+   },
+ });

  // takumi-js
- setGlyphCacheMaxBytes(16 * 1024 * 1024);
+ setCacheOptions({ budgets: { glyphs: 16 * 1024 * 1024 } });

  // types
- let options: FontCacheOptions;
+ let options: RendererOptions;
```
