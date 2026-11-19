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

# See what a renderer's cache holds

`Renderer`, `PdfRenderer`, and `Painter` have `cacheStats()`, which reports the bytes the pool holds and how lookups of each kind went.

```ts
const { memory, images } = renderer.cacheStats();
console.log(memory.current, memory.high);
console.log(images.hits, images.misses, images.evictions);
```

`takumi-js` exports `cacheStats` for the renderer that `render` and the other functions share. It returns `undefined` until the first render creates that renderer. In Rust, `Cache::stats` does the same for a `Cache`.
