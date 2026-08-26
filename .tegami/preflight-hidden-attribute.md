---
packages:
  "@takumi-rs/core":
    type: patch
  "@takumi-rs/wasm":
    type: patch
  "takumi-pdf":
    type: patch
---

### Hide elements carrying the `hidden` attribute

With Tailwind imported, `hidden` now hides an element the way Preflight does, including against an important utility.
