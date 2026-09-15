---
packages:
  "takumi": minor
  "takumi-pdf": minor
---

# Stop bundling the WOFF1 decoder in the wasm packages

`@takumi-rs/wasm` and `takumi-pdf` only ever advertised TTF, OTF and WOFF2 fonts, but pulled the WOFF1 decoder in through `takumi-bindings-common`. It is gone from both; `@takumi-rs/core` keeps WOFF1 support.
