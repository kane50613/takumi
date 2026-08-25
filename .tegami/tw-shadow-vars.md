---
packages:
  "@takumi-rs/core":
    type: minor
  "@takumi-rs/wasm":
    type: minor
  "takumi-pdf":
    type: minor
---

### Resolve shadow colours through CSS variables

`shadow-brand-500` and `text-shadow-brand-500` read `--color-brand-500` through `--tw-shadow-color` and `--tw-text-shadow-color`, with each layer's own colour as the fallback.
