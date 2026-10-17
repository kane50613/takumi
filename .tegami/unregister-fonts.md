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

Renderers now have `unregisterFont({ name })`, which removes every face of the family `name` with all its `unicodeRange` subsets, and `clearFonts()`, which goes back to the bundled fallback font. A family that `registerFont` returned can be passed back as is. A long-running renderer can let go of fonts it has seen and keep its other caches warm, and a later render that lists a released font registers it again. In Rust, `Fonts::unregister_family` does the same.
