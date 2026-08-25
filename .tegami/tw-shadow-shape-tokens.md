---
packages:
  "@takumi-rs/core":
    type: minor
  "@takumi-rs/wasm":
    type: minor
  "takumi-pdf":
    type: minor
---

### Resolve shadow preset shapes through CSS variables

`shadow-md`, `inset-shadow-sm` and `text-shadow-sm` now read `var(--shadow-md)`, `var(--inset-shadow-sm)` and `var(--text-shadow-sm)`, with the built-in layers as the fallback. A custom shape carries its own colours, so `shadow-*` colour utilities only reach the built-in fallback.
