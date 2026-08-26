---
packages:
  "@takumi-rs/core":
    type: minor
  "@takumi-rs/wasm":
    type: minor
  "takumi-pdf":
    type: minor
---

### Resolve blur and drop-shadow presets through CSS variables

`blur-md` now reads `var(--blur-md)` and `drop-shadow-md` reads `var(--drop-shadow-md)`, with the built-in shape as the fallback. Only the preset scale is themable; a token outside it stays unrecognized.
