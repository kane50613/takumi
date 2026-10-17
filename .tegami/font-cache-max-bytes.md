---
packages:
  "@takumi-rs/helpers": minor
  "@takumi-rs/core": minor
  "@takumi-rs/wasm": minor
  "takumi-paint": minor
  "takumi-pdf": minor
---

# Bound render fonts with `fontCacheMaxBytes`

Past this cap in decoded font bytes, a renderer releases the least recently used fonts that renders' `fonts` lists loaded, and a later render loads them again. Fonts from `registerFont` stay, and a running render keeps the fonts it started with. The wasm `Renderer` now passes its options, such as `cacheMaxBytes`, to the native renderer.
