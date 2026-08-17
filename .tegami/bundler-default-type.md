---
packages:
  "takumi-pdf":
    type: patch
---

### Type the bundler entries' default export

`export *` does not forward a default export, so the wasm init default was untyped on every bundler entry.
