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

`Renderer`, `PdfRenderer`, and `Painter` take a `cache` option. Its `maxBytes`, 128 MiB by default, is one pool for everything a renderer reuses. Setting `images`, `glyphs`, or `fonts` to `"none"` keeps none of that kind. The options apply when the renderer is created.

```ts
const renderer = new Renderer({
  cache: {
    maxBytes: 256 * 1024 * 1024,
    glyphs: "none",
  },
});
```

#### Migration

`cacheMaxBytes`, `fontCacheMaxBytes`, and `FontCacheOptions` are deprecated. In `@takumi-rs/core` and `@takumi-rs/wasm`, `setGlyphCacheMaxBytes` is deprecated, and only `0` still changes anything.

```diff
- setGlyphCacheMaxBytes(0);
- const renderer = new Renderer({ cacheMaxBytes: 64 * 1024 * 1024 });
+ const renderer = new Renderer({ cache: { maxBytes: 64 * 1024 * 1024, glyphs: "none" } });

  // types
- let options: FontCacheOptions;
+ let options: RendererOptions;
```
