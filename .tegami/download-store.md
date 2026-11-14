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

Every renderer keeps the images, `@font-face` files, and font URLs it downloads in a store keyed by URL, a `MemoryStore` of 32 MiB by default, so a later render skips the download. `cache.store` takes a `MemoryStore` of another size, any object with `get` and `set`, or `null` to keep nothing, and `setCacheOptions` replaces it. `cacheStats().store` reports its bytes, hits, misses, and evictions. A stored download serves a render only when its `allowUrl` accepts the URL a redirect led to, and renders share a download in flight only when their `allowUrl` and `maxBytes` match. `googleFonts` and `fontFromUrl` take `store` too, defaulting to one 32 MiB store for each `fetch` function, which replaces the CSS and emoji caches. `googleFonts` takes `text` to load one file holding just those characters, and falls back to `unicode-range` subsets past 6000 encoded characters. A font in a render's `fonts` list, and `googleFonts`, take `cache: "none"`, which drops a font the render loaded once it settles. `images.fetchCache` and a `Map` as `googleFonts`'s `cache` still work, and are deprecated. In Rust, `HeldFonts::is_held` tells whether a lease or a pin keeps a font file.
