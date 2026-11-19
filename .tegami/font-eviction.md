---
packages:
  takumi-core:
    type: minor
  takumi:
    type: minor
  "@takumi-rs/helpers":
    type: patch
  "@takumi-rs/core":
    type: minor
  "@takumi-rs/wasm":
    type: minor
  takumi-pdf:
    type: minor
  takumi-paint:
    type: minor
---

# Evict fonts one file at a time from the renderer's cache

Font files that renders load now share the renderer's `maxBytes` pool and leave it one file at a time, least recently used first. A family that keeps gaining files, such as one Google Fonts `text=` subset per title, now stays within `maxBytes`. A render keeps the fonts it uses until it finishes, and fonts from `registerFont` still stay. With `cache.fonts` set to `"none"`, each file goes as soon as the last render using it finishes.

```ts
const renderer = new Renderer({ cache: { fonts: "none" } });
```

`fontCacheMaxBytes` no longer bounds the fonts apart. A value of `0` still keeps none, as `"none"` does.

Among equally matching faces of one family, the one listed last now wins every time.
