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

`unregisterFont({ name })` removes a family with all its `unicodeRange` subsets, and `clearFonts()` goes back to the bundled fallback font. A later render that lists a released font registers it again. In Rust, use `Fonts::unregister_family`.
