---
packages:
  "@takumi-rs/helpers": minor
  "@takumi-rs/core": minor
  "@takumi-rs/wasm": minor
  "takumi-paint": minor
  "takumi-pdf": minor
---

# Bound the fonts a renderer keeps with `fontCacheMaxBytes`

Renderers take `fontCacheMaxBytes`, a budget in font file bytes for the fonts that renders' `fonts` lists registered. Past it, a renderer releases the families used least recently, skipping any a render in progress lists in `fonts` or `fontFamilies`, and registers them again when a later render lists them. A family that any `registerFont` call added to is never released this way. The wasm `Renderer` now also passes its other options, such as `cacheMaxBytes`, to the native renderer.
