---
packages:
  "@takumi-rs/helpers": minor
  "@takumi-rs/core": minor
  "@takumi-rs/wasm": minor
  "takumi-paint": minor
  "takumi-pdf": minor
---

# Bound the fonts a renderer keeps with `fontCacheMaxBytes`

Renderers take `fontCacheMaxBytes`. Past it, a renderer releases the font families that renders' `fonts` lists registered and used least recently, never one a render in progress uses, and registers them again when a later render lists them. Fonts from `registerFont` are always kept. The wasm `Renderer` now also passes its other options, such as `cacheMaxBytes`, to the native renderer.
