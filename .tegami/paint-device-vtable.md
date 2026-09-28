---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Shrink the wasm packages

Borders, outlines, text and decorations now compile once for every output format instead of once per backend. `@takumi-rs/wasm` is about 18 KB smaller gzipped, and `takumi-pdf` about 7 KB.
