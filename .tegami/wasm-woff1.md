---
packages:
  "takumi": minor
  "takumi-pdf": minor
---

# Drop the WOFF1 decoder from the wasm packages

`@takumi-rs/wasm` and `takumi-pdf` no longer bundle the WOFF1 decoder; TTF, OTF and WOFF2 still load. `@takumi-rs/core` keeps WOFF1.
