---
packages:
  takumi-core:
    type: minor
  takumi:
    type: minor
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

# Keep downloaded bytes in a bounded store, and load Google Fonts for known text

Every renderer now keeps the images, `@font-face` files, and font URLs it downloads in a store keyed by URL, so a later render skips the download. By default, that store is a `MemoryStore` of 32 MiB.

```ts
import { Renderer } from "takumi-js/node";
import { googleFonts, MemoryStore } from "takumi-js/helpers";

const renderer = new Renderer({
  cache: { store: new MemoryStore({ maxBytes: 128 * 1024 * 1024 }) },
});

const fonts = googleFonts({ families: ["Noto Sans JP"], text: "東京の夜景" });
```

- `cache.store` takes a `MemoryStore` of another size, any object with `get` and `set`, or `null` to keep nothing. `setCacheOptions` replaces it.
- `cacheStats().store` reports the store's bytes, hits, misses, and evictions.
- A stored download serves a render only when its `allowUrl` accepts the URL a redirect led to. Renders share a download in flight only when their `allowUrl` and `maxBytes` match.
- `googleFonts` and `fontFromUrl` take `store` too. By default, they share one 32 MiB store for each `fetch` function, which replaces the CSS and emoji caches.
- `googleFonts` takes `text` to load one file holding just those characters. Past 6000 encoded characters, it loads the `unicode-range` subsets instead.
- A font in a render's `fonts` list, and `googleFonts`, take `cache: "none"`, which drops a font the render loaded once the render settles.
- In Rust, `HeldFonts::is_held` tells whether a lease or a pin keeps a font file.

#### Migration

`images.fetchCache` and a `Map` as the `cache` of `googleFonts` still work, and are deprecated.

```diff
- await renderer.render(node, { images: { fetchCache } });
+ const renderer = new Renderer({ cache: { store: new MemoryStore() } });

- googleFonts({ families, cache: new Map() });
+ googleFonts({ families, store: new MemoryStore() });
```
