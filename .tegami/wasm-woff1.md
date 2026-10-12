---
packages:
  "takumi": minor
  "takumi-pdf": minor
---

# Drop WOFF1 from the wasm packages

`@takumi-rs/wasm` and `takumi-pdf` load TTF, OTF, and WOFF2, but not WOFF1. `@takumi-rs/core` keeps WOFF1.
