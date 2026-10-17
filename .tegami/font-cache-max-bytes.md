---
packages:
  "@takumi-rs/helpers": minor
  "@takumi-rs/core": minor
  "@takumi-rs/wasm": minor
  "takumi-paint": minor
  "takumi-pdf": minor
---

# Bound render fonts with `fontCacheMaxBytes`

Past this cap in font file bytes, a renderer releases the least recently used fonts that renders' `fonts` lists loaded, and a later render loads them again. Fonts from `registerFont` and fonts a running render lists stay. The wasm `Renderer` now passes its options, such as `cacheMaxBytes`, to the native renderer.
