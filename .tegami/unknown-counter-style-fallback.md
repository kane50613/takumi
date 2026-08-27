---
packages:
  "@takumi-rs/core":
    type: patch
  "@takumi-rs/wasm":
    type: patch
  "takumi-pdf":
    type: patch
---

### Render unknown counter styles as `decimal`

`list-style-type` now accepts any counter style name. Takumi renders unsupported styles with `decimal` markers. This matches browser behavior.
