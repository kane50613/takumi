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

# Reuse downloaded bytes across renders, and load Google Fonts for known text

Renderers now keep the images, `@font-face` files, and font URLs they download in a 32 MiB store keyed by URL, so a later render skips the download. A render that passes its own `fetch` skips the store, since that `fetch` may send credentials.

```ts
import { googleFonts } from "takumi-js/helpers";

const fonts = googleFonts({ families: ["Noto Sans JP"], text: "東京の夜景" });
```

- `googleFonts` takes `text` to load one file holding just those characters.
- A font in a render's `fonts` list takes `cache: "none"`, which drops the font once the render settles. `googleFonts` sets it on every subset through `cacheMode: "none"`.
- Under the default `"auto"`, a `googleFonts` subset loaded for `text` drops once its render settles too, since another render rarely asks for the same characters. Range subsets and other fonts stay as before.
- `CacheMode` names the `"auto" | "none"` policy that images and fonts share. `ImageCacheMode` stays as a deprecated alias.

#### Migration

`images.fetchCache` is deprecated, since renders keep their downloads without it.

```diff
- await renderer.render(node, { images: { fetchCache } });
+ await renderer.render(node);
```
