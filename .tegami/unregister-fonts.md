---
packages:
  "takumi-core": minor
  "takumi": minor
  "@takumi-rs/helpers": minor
  "@takumi-rs/core": minor
  "@takumi-rs/wasm": minor
  "takumi-paint": minor
  "takumi-pdf": minor
---

# Release fonts a renderer no longer needs

Renderers now have `unregisterFont(family)`, which removes a family with all its `unicodeRange` subsets, and `clearFonts()`, which goes back to the bundled fallback font. A long-running renderer can let go of fonts it has seen and keep its other caches warm. A render that lists a released font in `fonts` registers it again. In Rust, `Fonts::unregister` does the same.
